package dev.game.training;

import android.Manifest;
import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.content.Intent;
import android.net.ConnectivityManager;
import android.net.Network;
import android.net.NetworkCapabilities;
import android.os.Build;
import android.os.VibrationEffect;
import android.os.Vibrator;
import android.view.WindowManager;
import android.webkit.WebView;
import androidx.activity.ComponentActivity;
import androidx.activity.OnBackPressedCallback;
import com.getcapacitor.JSObject;
import com.getcapacitor.PermissionState;
import com.getcapacitor.Plugin;
import com.getcapacitor.PluginCall;
import com.getcapacitor.PluginMethod;
import com.getcapacitor.annotation.CapacitorPlugin;
import com.getcapacitor.annotation.Permission;
import com.getcapacitor.annotation.PermissionCallback;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

@CapacitorPlugin(
    name = "GamePlatform",
    permissions = {
      @Permission(
          alias = "notifications",
          strings = {Manifest.permission.POST_NOTIFICATIONS})
    })
public final class GamePlatformPlugin extends Plugin {
  private static final String CHANNEL = "duel-reminders";
  private static final int REMINDER = 47001;
  // One process-wide queue also orders a pending write before a recreated Activity's first read.
  static final ExecutorService STORAGE = Executors.newSingleThreadExecutor();
  private static final ExecutorService UPDATES = Executors.newSingleThreadExecutor();
  private SessionVault vault;
  private ConnectivityManager connectivity;
  private ConnectivityManager.NetworkCallback network;
  private boolean duel;
  private boolean online;
  private boolean awake;
  private boolean haptics;
  private boolean reminders;
  private String title = "Spellgambit";
  private String body = "Return to your duel within 60 seconds.";

  @Override
  public void load() {
    vault = new SessionVault(getContext());
    connectivity = getContext().getSystemService(ConnectivityManager.class);
    network =
        new ConnectivityManager.NetworkCallback() {
          private void publish() {
            Network active = connectivity.getActiveNetwork();
            NetworkCapabilities caps =
                active == null ? null : connectivity.getNetworkCapabilities(active);
            boolean connected =
                caps != null && caps.hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET);
            int identity = active == null ? 0 : active.hashCode();
            if (lastNetwork == identity && lastConnected != null && lastConnected == connected)
              return;
            lastNetwork = identity;
            lastConnected = connected;
            JSObject value = new JSObject();
            value.put("connected", connected);
            notifyListeners("network", value);
          }

          @Override
          public void onAvailable(Network value) {
            publish();
          }

          @Override
          public void onLost(Network value) {
            publish();
          }

          @Override
          public void onCapabilitiesChanged(Network value, NetworkCapabilities caps) {
            publish();
          }
        };
    connectivity.registerDefaultNetworkCallback(network);
    if (getActivity() instanceof ComponentActivity) {
      ((ComponentActivity) getActivity())
          .getOnBackPressedDispatcher()
          .addCallback(
              (ComponentActivity) getActivity(),
              new OnBackPressedCallback(true) {
                @Override
                public void handleOnBackPressed() {
                  if (hasListeners("back")) notifyListeners("back", new JSObject());
                  else getActivity().moveTaskToBack(true);
                }
              });
    }
    NotificationManager manager = getContext().getSystemService(NotificationManager.class);
    NotificationChannel channel =
        new NotificationChannel(CHANNEL, "Spellgambit", NotificationManager.IMPORTANCE_DEFAULT);
    channel.setDescription("Optional reminders while a duel is running");
    manager.createNotificationChannel(channel);
  }

  private int lastNetwork = -1;
  private Boolean lastConnected;

  @PluginMethod
  public void loadSession(PluginCall call) {
    STORAGE.execute(
        () -> {
          SessionVault.Stored stored = vault.load();
          JSObject result = new JSObject();
          result.put("token", stored == null ? null : stored.token);
          result.put("rotatedAt", stored == null ? 0 : stored.rotatedAt);
          call.resolve(result);
        });
  }

  @PluginMethod
  public void saveSession(PluginCall call) {
    String token = call.getString("token");
    long rotatedAt = call.getLong("rotatedAt", 0L);
    STORAGE.execute(
        () -> {
          try {
            vault.save(token, rotatedAt);
            call.resolve();
          } catch (Exception error) {
            call.reject("SECURE_STORAGE");
          }
        });
  }

  @PluginMethod
  public void clearSession(PluginCall call) {
    STORAGE.execute(
        () -> {
          vault.clear();
          call.resolve();
        });
  }

  @PluginMethod
  public void metadata(PluginCall call) {
    JSObject value = new JSObject();
    value.put("nativeVersion", BuildConfig.VERSION_NAME);
    value.put("bundleVersion", ((MainActivity) getActivity()).bundles.current());
    value.put("bundleTrial", ((MainActivity) getActivity()).bundles.trial());
    value.put(
        "storeUrl",
        BuildConfig.DEBUG
            ? ""
            : "https://play.google.com/store/apps/details?id=" + BuildConfig.APPLICATION_ID);
    value.put(
        "webViewVersion",
        WebView.getCurrentWebViewPackage() == null
            ? "0"
            : WebView.getCurrentWebViewPackage().versionName);
    value.put(
        "notificationsAllowed",
        Build.VERSION.SDK_INT < 33
            || getPermissionState("notifications") == PermissionState.GRANTED);
    value.put(
        "screenAwake",
        (getActivity().getWindow().getAttributes().flags
                & WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
            != 0);
    call.resolve(value);
  }

  @PluginMethod
  public void configure(PluginCall call) {
    duel = call.getBoolean("duel", false);
    online = call.getBoolean("online", false);
    awake = call.getBoolean("awake", false);
    haptics = call.getBoolean("haptics", false);
    reminders = call.getBoolean("reminders", false);
    title = call.getString("title", "Spellgambit");
    body = call.getString("body", "Return to your duel within 60 seconds.");
    if (title.length() > 80) title = title.substring(0, 80);
    if (body.length() > 300) body = body.substring(0, 300);
    getActivity()
        .runOnUiThread(
            () -> {
              if (duel && awake)
                getActivity().getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
              else
                getActivity()
                    .getWindow()
                    .clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
              call.resolve();
            });
  }

  @PluginMethod
  public void enableReminders(PluginCall call) {
    if (Build.VERSION.SDK_INT < 33
        || getPermissionState("notifications") == PermissionState.GRANTED) {
      JSObject value = new JSObject();
      value.put("allowed", true);
      call.resolve(value);
    } else requestPermissionForAlias("notifications", call, "notificationPermission");
  }

  @PermissionCallback
  private void notificationPermission(PluginCall call) {
    JSObject value = new JSObject();
    value.put("allowed", getPermissionState("notifications") == PermissionState.GRANTED);
    call.resolve(value);
  }

  @PluginMethod
  public void haptic(PluginCall call) {
    if (haptics) {
      String kind = call.getString("kind", "hit");
      Vibrator vibrator = getContext().getSystemService(Vibrator.class);
      if (vibrator != null && vibrator.hasVibrator()) {
        long duration = kind.equals("end") ? 75 : kind.equals("combo") ? 45 : 18;
        vibrator.vibrate(
            VibrationEffect.createOneShot(duration, VibrationEffect.DEFAULT_AMPLITUDE));
      }
    }
    call.resolve();
  }

  @PluginMethod
  public void minimize(PluginCall call) {
    getActivity()
        .runOnUiThread(
            () -> {
              getActivity().moveTaskToBack(true);
              call.resolve();
            });
  }

  @PluginMethod
  public void checkBundle(PluginCall call) {
    UPDATES.execute(
        () -> {
          try {
            BundleStore store = ((MainActivity) getActivity()).bundles;
            store.check(call.getBoolean("force", false));
            JSObject value = new JSObject();
            value.put("pending", store.pending());
            value.put("enabled", store.enabled());
            value.put("needsNative", store.needsNative());
            call.resolve(value);
          } catch (Exception ignored) {
            call.reject("UPDATE_FAILED");
          }
        });
  }

  @PluginMethod
  public void applyBundle(PluginCall call) {
    getActivity()
        .runOnUiThread(
            () -> {
              try {
                if (duel || !call.getBoolean("idle", false)) {
                  call.reject("BUSY");
                  return;
                }
                MainActivity activity = (MainActivity) getActivity();
                String path = activity.bundles.apply(true);
                call.resolve();
                if (path != null) activity.loadBundle(path);
              } catch (Exception ignored) {
                call.reject("UPDATE_FAILED");
              }
            });
  }

  @PluginMethod
  public void confirmBundle(PluginCall call) {
    getActivity()
        .runOnUiThread(
            () -> {
              try {
                ((MainActivity) getActivity()).confirmBundle();
                call.resolve();
              } catch (Exception ignored) {
                call.reject("UPDATE_FAILED");
              }
            });
  }

  @Override
  protected void handleOnPause() {
    notifyListeners("pause", new JSObject());
    if (!duel
        || !online
        || !reminders
        || (Build.VERSION.SDK_INT >= 33
            && getPermissionState("notifications") != PermissionState.GRANTED)) return;
    Intent open =
        new Intent(getContext(), MainActivity.class)
            .addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP | Intent.FLAG_ACTIVITY_CLEAR_TOP);
    PendingIntent intent =
        PendingIntent.getActivity(
            getContext(),
            REMINDER,
            open,
            PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE);
    Notification notification =
        new Notification.Builder(getContext(), CHANNEL)
            .setSmallIcon(R.drawable.ic_duel_notification)
            .setContentTitle(title)
            .setContentText(body)
            .setContentIntent(intent)
            .setAutoCancel(true)
            .setVisibility(Notification.VISIBILITY_PRIVATE)
            .setTimeoutAfter(60000)
            .build();
    getContext().getSystemService(NotificationManager.class).notify(REMINDER, notification);
  }

  @Override
  protected void handleOnResume() {
    getContext().getSystemService(NotificationManager.class).cancel(REMINDER);
    notifyListeners("resume", new JSObject());
  }

  @Override
  protected void handleOnDestroy() {
    if (connectivity != null && network != null) connectivity.unregisterNetworkCallback(network);
  }
}
