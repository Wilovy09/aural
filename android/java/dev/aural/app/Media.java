package dev.aural.app;

import android.Manifest;
import android.app.Activity;
import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.content.IntentFilter;
import android.content.pm.PackageManager;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.media.MediaMetadata;
import android.media.session.MediaSession;
import android.media.session.PlaybackState;
import android.os.Build;

import java.util.concurrent.ConcurrentLinkedQueue;

/**
 * Android's view of what Aural plays: a media session (the player in quick settings, on the lock
 * screen and for headset buttons) and its notification, kept up by a foreground service so the
 * music goes on in the background. The buttons pressed there wait in a queue the engine reads.
 */
public final class Media {
    private Media() {}

    static final int NOTIFICATION = 1;
    private static final String CHANNEL = "playback";
    private static final String ACTION = "dev.aural.app.MEDIA";

    private static final ConcurrentLinkedQueue<String> PRESSED = new ConcurrentLinkedQueue<>();

    private static MediaSession session;
    private static boolean receiving;
    private static boolean asked;
    private static boolean serving;
    /** The notification the service shows, the latest one built. */
    static Notification notification;

    /** Shows `title` by `artist` as playing or paused at `position` of `duration` ms. */
    public static void update(final Activity activity, final String title, final String artist,
            final byte[] art, final long duration, final long position, final boolean playing) {
        activity.runOnUiThread(new Runnable() {
            @Override
            public void run() {
                show(activity, title, artist, art, duration, position, playing);
            }
        });
    }

    /** Takes the session and the notification down, when nothing is left to play. */
    public static void stop(final Activity activity) {
        activity.runOnUiThread(new Runnable() {
            @Override
            public void run() {
                if (serving) {
                    activity.stopService(new Intent(activity, PlaybackService.class));
                    serving = false;
                }
                manager(activity).cancel(NOTIFICATION);
                if (session != null) {
                    session.setActive(false);
                }
            }
        });
    }

    /** The next button pressed ("play", "pause", "next", "previous", "seek:<ms>"), or null. */
    public static String take() {
        return PRESSED.poll();
    }

    private static void show(Activity activity, String title, String artist, byte[] art,
            long duration, long position, boolean playing) {
        ask(activity);
        MediaSession media = session(activity);
        Bitmap cover = art == null ? null : BitmapFactory.decodeByteArray(art, 0, art.length);

        MediaMetadata.Builder metadata = new MediaMetadata.Builder()
                .putString(MediaMetadata.METADATA_KEY_TITLE, title)
                .putString(MediaMetadata.METADATA_KEY_ARTIST, artist)
                .putLong(MediaMetadata.METADATA_KEY_DURATION, duration);
        if (cover != null) {
            metadata.putBitmap(MediaMetadata.METADATA_KEY_ALBUM_ART, cover);
        }
        media.setMetadata(metadata.build());
        media.setPlaybackState(new PlaybackState.Builder()
                .setActions(PlaybackState.ACTION_PLAY | PlaybackState.ACTION_PAUSE
                        | PlaybackState.ACTION_PLAY_PAUSE | PlaybackState.ACTION_SKIP_TO_NEXT
                        | PlaybackState.ACTION_SKIP_TO_PREVIOUS | PlaybackState.ACTION_SEEK_TO)
                .setState(playing ? PlaybackState.STATE_PLAYING : PlaybackState.STATE_PAUSED,
                        position, playing ? 1f : 0f)
                .build());
        media.setActive(true);

        Intent open = activity.getPackageManager()
                .getLaunchIntentForPackage(activity.getPackageName());
        Notification.Builder builder = new Notification.Builder(activity, channel(activity))
                .setSmallIcon(icon(activity))
                .setContentTitle(title)
                .setContentText(artist)
                .setVisibility(Notification.VISIBILITY_PUBLIC)
                .setOngoing(playing)
                .setShowWhen(false)
                .setContentIntent(PendingIntent.getActivity(activity, 0, open,
                        PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT))
                .addAction(button(activity, android.R.drawable.ic_media_previous, "Anterior",
                        "previous", 1))
                .addAction(playing
                        ? button(activity, android.R.drawable.ic_media_pause, "Pausa", "pause", 2)
                        : button(activity, android.R.drawable.ic_media_play, "Reproducir", "play", 3))
                .addAction(button(activity, android.R.drawable.ic_media_next, "Siguiente",
                        "next", 4))
                .setStyle(new Notification.MediaStyle()
                        .setMediaSession(media.getSessionToken())
                        .setShowActionsInCompactView(0, 1, 2));
        if (cover != null) {
            builder.setLargeIcon(cover);
        }
        notification = builder.build();

        if (!serving) {
            activity.startForegroundService(new Intent(activity, PlaybackService.class));
            serving = true;
        } else {
            manager(activity).notify(NOTIFICATION, notification);
        }
    }

    /** Asks once for leave to post notifications, which Android 13 and later want. */
    private static void ask(Activity activity) {
        if (asked || Build.VERSION.SDK_INT < 33) {
            return;
        }
        asked = true;
        if (activity.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS)
                != PackageManager.PERMISSION_GRANTED) {
            activity.requestPermissions(new String[] {Manifest.permission.POST_NOTIFICATIONS}, 7);
        }
    }

    private static MediaSession session(Activity activity) {
        if (session != null) {
            return session;
        }
        session = new MediaSession(activity.getApplicationContext(), "Aural");
        session.setCallback(new MediaSession.Callback() {
            @Override
            public void onPlay() {
                PRESSED.add("play");
            }

            @Override
            public void onPause() {
                PRESSED.add("pause");
            }

            @Override
            public void onStop() {
                PRESSED.add("pause");
            }

            @Override
            public void onSkipToNext() {
                PRESSED.add("next");
            }

            @Override
            public void onSkipToPrevious() {
                PRESSED.add("previous");
            }

            @Override
            public void onSeekTo(long position) {
                PRESSED.add("seek:" + position);
            }
        });
        return session;
    }

    /** A notification button that hands `command` to the queue through a broadcast. */
    private static Notification.Action button(Activity activity, int icon, String label,
            String command, int code) {
        receive(activity);
        Intent intent = new Intent(ACTION).setPackage(activity.getPackageName())
                .putExtra("command", command);
        PendingIntent pending = PendingIntent.getBroadcast(activity, code, intent,
                PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        return new Notification.Action.Builder(icon, label, pending).build();
    }

    private static void receive(Activity activity) {
        if (receiving) {
            return;
        }
        receiving = true;
        BroadcastReceiver receiver = new BroadcastReceiver() {
            @Override
            public void onReceive(Context context, Intent intent) {
                String command = intent.getStringExtra("command");
                if (command != null) {
                    PRESSED.add(command);
                }
            }
        };
        Context context = activity.getApplicationContext();
        if (Build.VERSION.SDK_INT >= 33) {
            context.registerReceiver(receiver, new IntentFilter(ACTION),
                    Context.RECEIVER_NOT_EXPORTED);
        } else {
            context.registerReceiver(receiver, new IntentFilter(ACTION));
        }
    }

    private static String channel(Activity activity) {
        NotificationChannel channel = new NotificationChannel(CHANNEL, "Reproducción",
                NotificationManager.IMPORTANCE_LOW);
        channel.setShowBadge(false);
        manager(activity).createNotificationChannel(channel);
        return CHANNEL;
    }

    /** The small icon: the logo's white symbol. */
    private static int icon(Activity activity) {
        int id = activity.getResources().getIdentifier("icon_small", "drawable",
                activity.getPackageName());
        return id != 0 ? id : android.R.drawable.ic_media_play;
    }

    private static NotificationManager manager(Context context) {
        return (NotificationManager) context.getSystemService(Context.NOTIFICATION_SERVICE);
    }
}
