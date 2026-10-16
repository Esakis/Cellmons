package dev.game.training;

import android.content.Context;
import android.util.AtomicFile;
import android.util.Base64;
import java.io.*;
import java.net.*;
import java.nio.charset.StandardCharsets;
import javax.net.ssl.HttpsURLConnection;
import org.json.JSONObject;

/** Private no-backup storage. Only a verified package can become a trial release. */
final class BundleStore {
  private static BundleStore instance;

  static synchronized BundleStore get(Context context) {
    if (instance == null) instance = new BundleStore(context.getApplicationContext());
    return instance;
  }

  private final File root;
  private final AtomicFile stateFile;
  private BundleState state = new BundleState(0, 0, 0, 0, false);
  private long checkedAt;
  private long forcedAt;
  private boolean needsNative;
  private int installingVersion;

  BundleStore(Context context) {
    root = new File(context.getNoBackupFilesDir(), "web-bundles");
    stateFile = new AtomicFile(new File(context.getNoBackupFilesDir(), "web-bundle-state.json"));
    try (InputStream input = stateFile.openRead()) {
      JSONObject value = new JSONObject(new String(read(input, 4096), StandardCharsets.UTF_8));
      if (BuildConfig.WEB_BUNDLE_KEY.equals(value.optString("key")))
        state =
            new BundleState(
                value.getInt("current"),
                value.getInt("previous"),
                value.getInt("pending"),
                value.getInt("highest"),
                value.getBoolean("trial"));
    } catch (Exception ignored) {
      /* Corrupt metadata falls back to bundled assets. */
    }
    state.highest = Math.max(state.highest, BuildConfig.WEB_BUNDLE_BASE_VERSION);
    if (state.pending <= state.highest) state.pending = 0;
    if (state.current <= BuildConfig.WEB_BUNDLE_BASE_VERSION) {
      state.current = 0;
      state.previous = 0;
      state.trial = false;
    }
  }

  synchronized String boot() throws IOException {
    state.boot();
    save();
    return currentPath();
  }

  private String currentPath() {
    File current = new File(root, Integer.toString(state.current));
    return state.current > 0 && new File(current, "index.html").isFile()
        ? current.getAbsolutePath()
        : null;
  }

  synchronized String apply(boolean idle) throws IOException {
    if (!state.apply(idle)) return null;
    save();
    return currentPath();
  }

  synchronized void confirm() throws IOException {
    state.confirm();
    save();
    cleanup();
  }

  synchronized String rollback() throws IOException {
    state.boot();
    save();
    return currentPath();
  }

  synchronized boolean trial() {
    return state.trial;
  }

  synchronized int pending() {
    return state.pending;
  }

  synchronized int current() {
    return state.current;
  }

  synchronized boolean needsNative() {
    return needsNative;
  }

  boolean enabled() {
    return !BuildConfig.WEB_BUNDLE_URL.isEmpty() && !BuildConfig.WEB_BUNDLE_KEY.isEmpty();
  }

  void check(boolean force) throws Exception {
    final int highest;
    synchronized (this) {
      long now = System.currentTimeMillis();
      if (!enabled()
          || (force ? now - forcedAt < 5 * 60 * 1000L : now - checkedAt < 6 * 60 * 60 * 1000L))
        return;
      if (force) forcedAt = now;
      checkedAt = System.currentTimeMillis();
      highest = Math.max(state.highest, Math.max(state.current, state.pending));
    }
    URL base = new URL(BuildConfig.WEB_BUNDLE_URL);
    if (!base.getProtocol().equals("https")
        || base.getUserInfo() != null
        || base.getQuery() != null
        || !base.getPath().endsWith("/")) throw new IOException("INVALID_UPDATE_ORIGIN");
    JSONObject value;
    try (InputStream input = connection(new URL(base, "manifest.json")).getInputStream()) {
      value = new JSONObject(new String(read(input, 8192), StandardCharsets.UTF_8));
    }
    BundlePackage.Manifest manifest =
        new BundlePackage.Manifest(
            value.getInt("version"),
            value.getInt("min_native_version"),
            value.getInt("protocol"),
            value.getString("content_version"),
            value.getString("sha256"),
            value.getLong("size"),
            Base64.decode(value.getString("signature"), Base64.NO_WRAP));
    if (manifest.version <= highest) return;
    manifest.verify(
        Base64.decode(BuildConfig.WEB_BUNDLE_KEY, Base64.NO_WRAP), Integer.MAX_VALUE, 1, highest);
    synchronized (this) {
      needsNative = manifest.minNative > BuildConfig.VERSION_CODE;
      if (needsNative) return;
    }
    if (!root.mkdirs() && !root.isDirectory()) throw new IOException("STORAGE_FAILED");
    File archive = new File(root, "download.zip"),
        target = new File(root, Integer.toString(manifest.version));
    synchronized (this) {
      installingVersion = manifest.version;
    }
    try {
      try (InputStream input =
              connection(new URL(base, manifest.version + ".zip")).getInputStream();
          OutputStream out = new FileOutputStream(archive)) {
        byte[] bytes = new byte[8192];
        long total = 0;
        int n;
        long deadline = System.nanoTime() + 120_000_000_000L;
        while ((n = input.read(bytes)) != -1) {
          total += n;
          if (total > manifest.size || System.nanoTime() > deadline)
            throw new IOException("DOWNLOAD_TOO_LARGE");
          out.write(bytes, 0, n);
        }
      }
      BundlePackage.verifyArchive(archive, manifest);
      if (target.exists()) BundlePackage.delete(target);
      BundlePackage.unpack(archive, target);
      synchronized (this) {
        state.pending = manifest.version;
        save();
        cleanup();
      }
    } finally {
      synchronized (this) {
        installingVersion = 0;
      }
      BundlePackage.delete(archive);
    }
  }

  private void cleanup() throws IOException {
    File[] files = root.listFiles();
    if (files == null) return;
    for (File file : files)
      if (file.isDirectory()
          && !file.getName().equals(Integer.toString(installingVersion))
          && !file.getName().equals(Integer.toString(state.current))
          && !file.getName().equals(Integer.toString(state.previous))
          && !file.getName().equals(Integer.toString(state.pending))) BundlePackage.delete(file);
  }

  private void save() throws IOException {
    FileOutputStream out = null;
    try {
      JSONObject value = new JSONObject();
      value.put("key", BuildConfig.WEB_BUNDLE_KEY);
      value.put("current", state.current);
      value.put("previous", state.previous);
      value.put("pending", state.pending);
      value.put("highest", state.highest);
      value.put("trial", state.trial);
      out = stateFile.startWrite();
      out.write(value.toString().getBytes(StandardCharsets.UTF_8));
      stateFile.finishWrite(out);
    } catch (Exception error) {
      if (out != null) stateFile.failWrite(out);
      throw new IOException("STORAGE_FAILED");
    }
  }

  private static HttpsURLConnection connection(URL url) throws IOException {
    HttpsURLConnection connection = (HttpsURLConnection) url.openConnection();
    connection.setInstanceFollowRedirects(false);
    connection.setConnectTimeout(10000);
    connection.setReadTimeout(15000);
    if (connection.getResponseCode() != 200) {
      connection.disconnect();
      throw new IOException("UPDATE_HTTP_FAILED");
    }
    return connection;
  }

  private static byte[] read(InputStream input, int limit) throws IOException {
    ByteArrayOutputStream out = new ByteArrayOutputStream();
    byte[] buffer = new byte[1024];
    int n;
    while ((n = input.read(buffer)) != -1) {
      if (out.size() + n > limit) throw new IOException("TOO_LARGE");
      out.write(buffer, 0, n);
    }
    return out.toByteArray();
  }
}
