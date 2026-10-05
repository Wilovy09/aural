package dev.aural.app;

import android.app.Activity;
import android.view.WindowManager;

/** Keeps the screen on while music plays, on the activity's UI thread. */
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
}
