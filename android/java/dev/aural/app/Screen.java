package dev.aural.app;

import android.app.Activity;
import android.os.Build;
import android.view.WindowInsets;
import android.view.WindowManager;

/** Keeps the screen on while music plays, and reports the room the system bars take. */
public final class Screen {
    private Screen() {}

    /** Adds or clears FLAG_KEEP_SCREEN_ON. Safe to call from any thread. */
    public static void keepOn(final Activity activity, final boolean on) {
        activity.runOnUiThread(new Runnable() {
            @Override
            public void run() {
                if (on) {
                    activity.getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
                } else {
                    activity.getWindow().clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
                }
            }
        });
    }

    /**
     * The room the visible system bars take at the top and bottom of the window, in pixels, or
     * null before the window is laid out. The window is drawn edge to edge, under them.
     */
    public static int[] insets(Activity activity) {
        WindowInsets insets = activity.getWindow().getDecorView().getRootWindowInsets();
        if (insets == null) {
            return null;
        }
        if (Build.VERSION.SDK_INT >= 30) {
            android.graphics.Insets bars = insets.getInsets(
                    WindowInsets.Type.systemBars() | WindowInsets.Type.displayCutout());
            return new int[] {bars.top, bars.bottom};
        }
        return new int[] {insets.getSystemWindowInsetTop(), insets.getSystemWindowInsetBottom()};
    }
}
