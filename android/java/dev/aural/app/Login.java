package dev.aural.app;

import android.app.Activity;
import android.app.Dialog;
import android.content.DialogInterface;
import android.os.Handler;
import android.os.Looper;
import android.webkit.CookieManager;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;

/**
 * Google sign-in in a full-screen WebView over the native activity. Rust starts it with
 * {@link #open} and polls {@link #state} and {@link #take}; the cookie header is handed back
 * once the proof cookies show up for the YouTube domain, and the window closes itself.
 */
public final class Login {
    private static final String DOMAIN = "https://www.youtube.com";
    private static final String[] PROOF = {"SAPISID=", "__Secure-3PAPISID="};
    private static final long POLL_MS = 500;

    private static volatile String state = "idle";
    private static volatile String cookies;
    private static Dialog dialog;
    private static WebView view;

    private Login() {}

    /** Opens the sign-in window at {@code url}. Safe to call from any thread. */
    public static void open(final Activity activity, final String url) {
        cookies = null;
        state = "opening";
        activity.runOnUiThread(new Runnable() {
            @Override
            public void run() {
                try {
                    show(activity, url);
                } catch (Throwable error) {
                    state = "error: " + error;
                }
            }
        });
    }

    /** What the window is doing: idle, opening, the page it is on, done, cancelled or an error. */
    public static String state() {
        return state;
    }

    /** The cookie header once sign-in finished, handed out once. */
    public static String take() {
        String header = cookies;
        cookies = null;
        return header;
    }

    private static void show(Activity activity, String url) {
        final CookieManager jar = CookieManager.getInstance();
        jar.setAcceptCookie(true);
        jar.removeAllCookies(null);

        view = new WebView(activity);
        WebSettings settings = view.getSettings();
        settings.setJavaScriptEnabled(true);
        settings.setDomStorageEnabled(true);
        settings.setUserAgentString(settings.getUserAgentString()
                .replace("; wv)", ")")
                .replace(" Version/4.0", ""));
        jar.setAcceptThirdPartyCookies(view, true);
        view.setWebViewClient(new WebViewClient() {
            @Override
            public void onPageFinished(WebView page, String at) {
                state = "page: " + shorten(at);
            }
        });

        dialog = new Dialog(activity, android.R.style.Theme_DeviceDefault_NoActionBar_Fullscreen);
        dialog.setContentView(view);
        dialog.setOnCancelListener(new DialogInterface.OnCancelListener() {
            @Override
            public void onCancel(DialogInterface ignored) {
                state = "cancelled";
                close();
            }
        });
        dialog.show();
        view.requestFocus();
        view.loadUrl(url);
        state = "loading";

        final Handler handler = new Handler(Looper.getMainLooper());
        handler.postDelayed(new Runnable() {
            @Override
            public void run() {
                if (view == null) {
                    return;
                }
                String header = jar.getCookie(DOMAIN);
                if (header != null && proven(header)) {
                    cookies = header;
                    state = "done";
                    close();
                    return;
                }
                handler.postDelayed(this, POLL_MS);
            }
        }, POLL_MS);
    }

    private static boolean proven(String header) {
        for (String name : PROOF) {
            if (header.startsWith(name) || header.contains("; " + name)) {
                return true;
            }
        }
        return false;
    }

    private static void close() {
        if (dialog != null) {
            dialog.dismiss();
            dialog = null;
        }
        if (view != null) {
            view.destroy();
            view = null;
        }
    }

    private static String shorten(String url) {
        int query = url.indexOf('?');
        String bare = query < 0 ? url : url.substring(0, query);
        return bare.length() > 80 ? bare.substring(0, 80) : bare;
    }
}
