package dev.aural.app;

import android.app.Activity;
import android.content.Context;
import android.net.wifi.WifiManager;

/** Keeps Wi-Fi listening to multicast, which mDNS (finding the other Aurals) arrives on. */
public final class Network {
    private Network() {}

    private static WifiManager.MulticastLock lock;

    public static void multicast(Activity activity) {
        if (lock != null) {
            return;
        }
        WifiManager wifi = (WifiManager) activity.getApplicationContext()
                .getSystemService(Context.WIFI_SERVICE);
        if (wifi == null) {
            return;
        }
        lock = wifi.createMulticastLock("aural");
        lock.setReferenceCounted(false);
        lock.acquire();
    }
}
