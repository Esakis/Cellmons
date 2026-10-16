package dev.game.training;

import static org.junit.Assert.*;

import android.content.Context;
import androidx.test.platform.app.InstrumentationRegistry;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import org.junit.Test;

public class BundleStoreTest {
  @Test
  public void apkKeyRotationDiscardsAllBundlesAuthenticatedByPreviousKey() throws Exception {
    Context context = InstrumentationRegistry.getInstrumentation().getTargetContext();
    File state = new File(context.getNoBackupFilesDir(), "web-bundle-state.json");
    try {
      Files.write(
          state.toPath(),
          "{\"key\":\"revoked-fixture-key\",\"current\":999,\"previous\":998,\"pending\":1000,\"highest\":999,\"trial\":false}"
              .getBytes(StandardCharsets.UTF_8));
      BundleStore store = new BundleStore(context);
      assertEquals(0, store.current());
      assertEquals(0, store.pending());
      assertFalse(store.trial());
    } finally {
      Files.deleteIfExists(state.toPath());
    }
  }
}
