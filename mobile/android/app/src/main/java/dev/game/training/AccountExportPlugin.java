package dev.game.training;

import android.app.Activity;
import android.content.Intent;
import android.net.Uri;
import androidx.activity.result.ActivityResult;
import com.getcapacitor.JSObject;
import com.getcapacitor.Plugin;
import com.getcapacitor.PluginCall;
import com.getcapacitor.PluginMethod;
import com.getcapacitor.annotation.ActivityCallback;
import com.getcapacitor.annotation.CapacitorPlugin;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.atomic.AtomicBoolean;
import javax.net.ssl.HttpsURLConnection;

/** An explicit user export uses Android's document picker, with no storage permission. */
@CapacitorPlugin(name = "AccountExport")
public final class AccountExportPlugin extends Plugin {
  private static final ExecutorService IO = Executors.newSingleThreadExecutor();
  private final AtomicBoolean pending = new AtomicBoolean();

  @PluginMethod
  public void save(PluginCall call) {
    if (!pending.compareAndSet(false, true)) {
      call.reject("EXPORT_BUSY");
      return;
    }
    String session = call.getString("session", "");
    if (!session.matches("[a-f0-9]{64}")) {
      pending.set(false);
      call.reject("EXPORT_FAILED");
      return;
    }
    if (BuildConfig.API_ORIGIN.isEmpty()) {
      pending.set(false);
      call.reject("EXPORT_FAILED");
      return;
    }
    Intent intent =
        new Intent(Intent.ACTION_CREATE_DOCUMENT)
            .addCategory(Intent.CATEGORY_OPENABLE)
            .setType("application/json")
            .putExtra(Intent.EXTRA_TITLE, "spellgambit-data.json");
    getActivity()
        .runOnUiThread(
            () -> {
              try {
                startActivityForResult(call, intent, "destination");
              } catch (RuntimeException ignored) {
                pending.set(false);
                call.reject("EXPORT_FAILED");
              }
            });
  }

  @ActivityCallback
  private void destination(PluginCall call, ActivityResult result) {
    if (call == null) {
      pending.set(false);
      return;
    }
    Uri uri = result.getData() == null ? null : result.getData().getData();
    if (result.getResultCode() != Activity.RESULT_OK || uri == null) {
      pending.set(false);
      JSObject value = new JSObject();
      value.put("saved", false);
      call.resolve(value);
      return;
    }
    if (!"content".equals(uri.getScheme())) {
      pending.set(false);
      call.reject("EXPORT_FAILED");
      return;
    }
    GamePlatformPlugin.STORAGE.execute(
        () -> {
          SessionVault.Stored stored = new SessionVault(getContext()).load();
          IO.execute(() -> download(call, uri, stored));
        });
  }

  private void download(PluginCall call, Uri uri, SessionVault.Stored stored) {
    HttpsURLConnection connection = null;
    try {
      if (stored == null) throw new Exception("UNAUTHENTICATED");
      String fingerprint =
          BundlePackage.hex(
              MessageDigest.getInstance("SHA-256")
                  .digest(stored.token.getBytes(StandardCharsets.US_ASCII)));
      if (!fingerprint.equals(call.getString("session"))) throw new Exception("SESSION_CHANGED");
      URL url = new URL(BuildConfig.API_ORIGIN + "/api/me/export");
      if (!url.getProtocol().equals("https")) throw new Exception("INVALID_ORIGIN");
      connection = (HttpsURLConnection) url.openConnection();
      connection.setInstanceFollowRedirects(false);
      connection.setConnectTimeout(10000);
      connection.setReadTimeout(15000);
      connection.setRequestProperty("Authorization", "Bearer " + stored.token);
      connection.setRequestProperty("X-Native-Version", BuildConfig.VERSION_NAME);
      connection.setRequestProperty("Origin", "https://localhost");
      if (connection.getResponseCode() != 200) throw new Exception("EXPORT_FAILED");
      String type = connection.getContentType();
      if (type == null || !type.startsWith("application/json"))
        throw new Exception("INVALID_EXPORT");
      try (InputStream input = connection.getInputStream();
          OutputStream output = getContext().getContentResolver().openOutputStream(uri, "wt")) {
        if (output == null) throw new Exception("INVALID_DESTINATION");
        byte[] bytes = new byte[8192];
        int n;
        long total = 0, deadline = System.nanoTime() + 120_000_000_000L;
        while ((n = input.read(bytes)) != -1) {
          total += n;
          if (total > 128 * 1024 * 1024L || System.nanoTime() > deadline)
            throw new Exception("EXPORT_TOO_LARGE");
          output.write(bytes, 0, n);
        }
      }
      JSObject value = new JSObject();
      value.put("saved", true);
      call.resolve(value);
    } catch (Exception ignored) {
      call.reject("EXPORT_FAILED");
    } finally {
      if (connection != null) connection.disconnect();
      pending.set(false);
    }
  }
}
