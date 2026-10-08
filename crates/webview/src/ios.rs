//! The iOS backend: a `UIWindow` laid over the app's own, holding a bar with a Cancel button and
//! a `WKWebView` over a non-persistent data store. UIKit and WebKit are main-thread only, and so
//! is every call here; WebKit runs the cookie completion and UIKit the button's handler on the
//! main thread too, which is what lets their state sit in an `Rc`.
//!
//! `objc2-web-kit` only declares `WKWebView` for macOS, where it is an `NSView`. On iOS it is a
//! `UIView`, so the class and the few methods used here are declared below.

use std::cell::{Cell, RefCell};
use std::ptr::NonNull;
use std::rc::Rc;

use anyhow::{Context as _, Result};
use block2::RcBlock;
use objc2::rc::{Allocated, Retained};
use objc2::{MainThreadMarker, MainThreadOnly, extern_class, extern_methods};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_foundation::{NSArray, NSHTTPCookie, NSObject, NSString, NSURL, NSURLRequest};
use objc2_ui_kit::{
    UIAction, UIApplication, UIButton, UIButtonType, UIColor, UIControlState, UIFont, UILabel,
    UIResponder, UIScene, UISceneActivationState, UIView, UIViewAutoresizing, UIViewController,
    UIWindow, UIWindowLevelAlert, UIWindowScene,
};
use objc2_web_kit::{
    WKHTTPCookieStore, WKUserScript, WKUserScriptInjectionTime, WKWebViewConfiguration,
    WKWebsiteDataStore,
};

use crate::native::{Fetch, Reading};
use crate::{Cookie, Target};

/// Mobile Safari's own user agent. WebKit's default leaves out the `Version/… Safari/…` tail,
/// and Google refuses to sign in a browser it reads as embedded.
const USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1";

/// How tall the bar with the Cancel button is, under the status bar.
const BAR: f64 = 52.;

extern_class!(
    /// [Apple's documentation](https://developer.apple.com/documentation/webkit/wkwebview?language=objc)
    #[unsafe(super(UIView, UIResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[derive(Debug, PartialEq, Eq, Hash)]
    struct WKWebView;
);

// Objective-C's names, as `objc2` spells every binding.
#[allow(non_snake_case)]
impl WKWebView {
    extern_methods!(
        #[unsafe(method(initWithFrame:configuration:))]
        #[unsafe(method_family = init)]
        fn initWithFrame_configuration(
            this: Allocated<Self>,
            frame: CGRect,
            configuration: &WKWebViewConfiguration,
        ) -> Retained<Self>;

        #[unsafe(method(configuration))]
        #[unsafe(method_family = none)]
        fn configuration(&self) -> Retained<WKWebViewConfiguration>;

        #[unsafe(method(setCustomUserAgent:))]
        #[unsafe(method_family = none)]
        fn setCustomUserAgent(&self, agent: Option<&NSString>);

        #[unsafe(method(loadRequest:))]
        #[unsafe(method_family = none)]
        fn loadRequest(&self, request: &NSURLRequest) -> Option<Retained<NSObject>>;

        #[unsafe(method(URL))]
        #[unsafe(method_family = none)]
        fn URL(&self) -> Option<Retained<NSURL>>;
    );
}

pub(crate) fn supported() -> bool {
    true
}

pub(crate) struct Window {
    window: Retained<UIWindow>,
    /// The app's window, made key again once this one goes.
    app: Option<Retained<UIWindow>>,
    view: Retained<WKWebView>,
    cookies: Retained<WKHTTPCookieStore>,
    fetch: Rc<RefCell<Fetch>>,
    /// Whether the user cancelled or the app closed the window.
    dismissed: Rc<Cell<bool>>,
}

/// The scene the app is showing, the one to lay the window over.
fn scene(mtm: MainThreadMarker) -> Option<Retained<UIWindowScene>> {
    let scenes = UIApplication::sharedApplication(mtm).connectedScenes();
    let scenes: Vec<Retained<UIScene>> = scenes.iter().collect();
    let shown = scenes
        .iter()
        .find(|scene| scene.activationState() == UISceneActivationState::ForegroundActive)
        .or(scenes.first())?
        .clone();
    shown.downcast::<UIWindowScene>().ok()
}

fn gray(white: f64) -> Retained<UIColor> {
    UIColor::colorWithRed_green_blue_alpha(white, white, white, 1.)
}

impl Window {
    pub(crate) fn open(target: &Target) -> Result<Self> {
        let mtm =
            MainThreadMarker::new().context("the sign-in window has to open on the main thread")?;
        let url = NSURL::URLWithString(&NSString::from_str(&target.url))
            .context("cannot parse the sign-in url")?;
        let scene = scene(mtm).context("no window scene to show the sign-in in")?;
        let app = scene.keyWindow();
        let window = UIWindow::initWithWindowScene(UIWindow::alloc(mtm), &scene);
        let bounds = window.bounds();
        // The app's window has been laid out already, so it knows where the notch and the home
        // bar are; the new one does not until it is on the screen.
        let (top, bottom) = app.as_ref().map_or((0., 0.), |app| {
            let insets = app.safeAreaInsets();
            (insets.top, insets.bottom)
        });
        let (width, height) = (bounds.size.width, bounds.size.height);

        let root = UIViewController::new(mtm);
        let page = UIView::initWithFrame(UIView::alloc(mtm), bounds);
        page.setBackgroundColor(Some(&gray(0.04)));
        page.setAutoresizingMask(
            UIViewAutoresizing::FlexibleWidth | UIViewAutoresizing::FlexibleHeight,
        );

        // The bar: Cancel on the left, the title in the middle.
        let bar = UIView::initWithFrame(
            UIView::alloc(mtm),
            CGRect::new(CGPoint::new(0., top), CGSize::new(width, BAR)),
        );
        bar.setAutoresizingMask(UIViewAutoresizing::FlexibleWidth);
        let title = UILabel::initWithFrame(
            UILabel::alloc(mtm),
            CGRect::new(CGPoint::new(0., 0.), CGSize::new(width, BAR)),
        );
        title.setText(Some(&NSString::from_str(&target.title)));
        // SAFETY: the label and the colour and font are live UIKit objects on the main thread.
        unsafe {
            title.setTextColor(Some(&gray(0.98)));
            title.setFont(Some(&UIFont::boldSystemFontOfSize(16.)));
        }
        title.setTextAlignment(objc2_ui_kit::NSTextAlignment::Center);
        title.setAutoresizingMask(UIViewAutoresizing::FlexibleWidth);
        bar.addSubview(&title);

        let dismissed = Rc::new(Cell::new(false));
        let cancel = {
            let dismissed = dismissed.clone();
            let handler = RcBlock::new(move |_: NonNull<UIAction>| dismissed.set(true));
            let action = unsafe { UIAction::actionWithHandler(RcBlock::as_ptr(&handler), mtm) };
            let button =
                UIButton::buttonWithType_primaryAction(UIButtonType::System, Some(&action), mtm);
            button.setTitle_forState(
                Some(&NSString::from_str("Cancelar")),
                UIControlState::Normal,
            );
            button.setTitleColor_forState(Some(&gray(0.98)), UIControlState::Normal);
            button.setFrame(CGRect::new(CGPoint::new(8., 0.), CGSize::new(96., BAR)));
            button
        };
        bar.addSubview(&cancel);
        page.addSubview(&bar);

        // The view copies the configuration, so the store is read back off the view afterwards.
        let configuration = unsafe { WKWebViewConfiguration::new(mtm) };
        let store = unsafe { WKWebsiteDataStore::nonPersistentDataStore(mtm) };
        unsafe { configuration.setWebsiteDataStore(&store) };
        let view = WKWebView::initWithFrame_configuration(
            WKWebView::alloc(mtm),
            CGRect::new(
                CGPoint::new(0., top + BAR),
                CGSize::new(width, (height - top - BAR - bottom).max(0.)),
            ),
            &configuration,
        );
        view.setAutoresizingMask(
            UIViewAutoresizing::FlexibleWidth | UIViewAutoresizing::FlexibleHeight,
        );
        view.setCustomUserAgent(Some(&NSString::from_str(
            target.agent.as_deref().unwrap_or(USER_AGENT),
        )));
        if let Some(source) = &target.script {
            let script = unsafe {
                WKUserScript::initWithSource_injectionTime_forMainFrameOnly(
                    WKUserScript::alloc(mtm),
                    &NSString::from_str(source),
                    WKUserScriptInjectionTime::AtDocumentStart,
                    true,
                )
            };
            unsafe {
                view.configuration()
                    .userContentController()
                    .addUserScript(&script)
            };
        }
        let cookies = unsafe { view.configuration().websiteDataStore().httpCookieStore() };
        page.addSubview(&view);
        root.setView(Some(&page));
        window.setRootViewController(Some(&root));
        window.setWindowLevel(unsafe { UIWindowLevelAlert });
        // A scripted window is never looked at: it stays hidden, and WebKit runs it all the same.
        if !target.scripted() {
            window.makeKeyAndVisible();
        }
        view.loadRequest(&NSURLRequest::requestWithURL(&url));

        Ok(Self {
            window,
            app,
            view,
            cookies,
            fetch: Rc::new(RefCell::new(Fetch::Idle)),
            dismissed,
        })
    }

    pub(crate) fn closed(&self) -> bool {
        self.dismissed.get()
    }

    pub(crate) fn host(&self) -> Option<String> {
        let url = self.view.URL()?;
        url.host().map(|host| host.to_string())
    }

    /// Hands back a finished cookie fetch, or starts one when none is in flight.
    pub(crate) fn fetch(&mut self) -> Option<Vec<Cookie>> {
        let reading = self.fetch.borrow_mut().take();
        match reading {
            Reading::Done(cookies) => return Some(cookies),
            Reading::Waiting => return None,
            Reading::Start => {}
        }
        let slot = self.fetch.clone();
        let done = RcBlock::new(move |found: NonNull<NSArray<NSHTTPCookie>>| {
            let found = unsafe { found.as_ref() };
            let cookies = found
                .iter()
                .map(|cookie| Cookie {
                    name: cookie.name().to_string(),
                    value: cookie.value().to_string(),
                    domain: cookie.domain().to_string(),
                })
                .collect();
            *slot.borrow_mut() = Fetch::Done(cookies);
        });
        unsafe { self.cookies.getAllCookies(&done) };
        None
    }

    /// Navigates the view to `url`; a url that does not parse is ignored.
    pub(crate) fn load(&self, url: &str) {
        let Some(url) = NSURL::URLWithString(&NSString::from_str(url)) else {
            return;
        };
        self.view.loadRequest(&NSURLRequest::requestWithURL(&url));
    }

    pub(crate) fn close(&self) {
        self.dismissed.set(true);
        if self.window.isHidden() {
            return;
        }
        self.window.setHidden(true);
        if let Some(app) = &self.app {
            app.makeKeyWindow();
        }
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        self.close();
    }
}
