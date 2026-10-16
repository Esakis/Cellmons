package dev.game.training;

import android.app.AlertDialog;
import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.webkit.WebView;
import com.getcapacitor.BridgeActivity;
import com.getcapacitor.ServerPath;

public class MainActivity extends BridgeActivity {
  BundleStore bundles;
  private boolean foreground;
  private final Handler watchdog = new Handler(Looper.getMainLooper());
  private final Runnable rollback =
      () -> {
        try {
          if (bundles.trial()) loadBundle(bundles.rollback());
        } catch (Exception ignored) {
          getBridge().setServerAssetPath("public");
        }
      };

  @Override
  public void onCreate(Bundle savedInstanceState) {
    registerPlugin(GamePlatformPlugin.class);
    registerPlugin(AccountExportPlugin.class);
    super.onCreate(savedInstanceState);
  }

  @Override
  protected void load() {
    String version =
        WebView.getCurrentWebViewPackage() == null
            ? "0"
            : WebView.getCurrentWebViewPackage().versionName;
    int major = 0;
    try {
      major = Integer.parseInt(version.split("\\.")[0]);
    } catch (NumberFormatException ignored) {
    }
    if (major < 113) {
      new AlertDialog.Builder(this)
          .setTitle("Spellgambit")
          .setMessage(R.string.webview_update_required)
          .setPositiveButton(
              R.string.webview_update,
              (dialog, which) -> {
                startActivity(
                    new Intent(
                        Intent.ACTION_VIEW,
                        Uri.parse(
                            "https://play.google.com/store/apps/details?id=com.google.android.webview")));
                finish();
              })
          .setNegativeButton(android.R.string.cancel, (dialog, which) -> finish())
          .setOnCancelListener(dialog -> finish())
          .show();
      return;
    }
    bundles = BundleStore.get(this);
    String path = null;
    try {
      path = bundles.boot();
    } catch (Exception ignored) {
      /* Embedded bundle is always available. */
    }
    bridgeBuilder.setServerPath(
        new ServerPath(
            path == null ? ServerPath.PathType.ASSET_PATH : ServerPath.PathType.BASE_PATH,
            path == null ? "public" : path));
    super.load();
  }

  void loadBundle(String path) {
    if (path == null) getBridge().setServerAssetPath("public");
    else getBridge().setServerBasePath(path);
    watchdog.removeCallbacks(rollback);
    if (foreground && bundles.trial()) watchdog.postDelayed(rollback, 15000);
  }

  void confirmBundle() throws Exception {
    bundles.confirm();
    watchdog.removeCallbacks(rollback);
  }

  @Override
  public void onResume() {
    super.onResume();
    foreground = true;
    if (bundles != null && bundles.trial()) watchdog.postDelayed(rollback, 15000);
  }

  @Override
  public void onPause() {
    foreground = false;
    watchdog.removeCallbacks(rollback);
    super.onPause();
  }

  @Override
  public void onDestroy() {
    watchdog.removeCallbacks(rollback);
    super.onDestroy();
  }
}
