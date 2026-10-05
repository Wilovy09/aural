package dev.aural.app;

import android.app.Service;
import android.content.Intent;
import android.content.pm.ServiceInfo;
import android.os.Build;
import android.os.IBinder;

/** Keeps Aural in the foreground while music plays, showing the media notification. */
public final class PlaybackService extends Service {
    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        if (Media.notification != null) {
            if (Build.VERSION.SDK_INT >= 29) {
                startForeground(Media.NOTIFICATION, Media.notification,
                        ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK);
            } else {
                startForeground(Media.NOTIFICATION, Media.notification);
            }
        }
        return START_NOT_STICKY;
    }

    @Override
    public IBinder onBind(Intent intent) {
        return null;
    }
}
