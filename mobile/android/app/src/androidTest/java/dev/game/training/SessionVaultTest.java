package dev.game.training;

import static org.junit.Assert.*;

import android.content.Context;
import androidx.test.ext.junit.runners.AndroidJUnit4;
import androidx.test.platform.app.InstrumentationRegistry;
import java.io.File;
import java.io.FileOutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.Arrays;
import org.junit.After;
import org.junit.Test;
import org.junit.runner.RunWith;

@RunWith(AndroidJUnit4.class)
public class SessionVaultTest {
  private final Context context = InstrumentationRegistry.getInstrumentation().getTargetContext();
  private final SessionVault vault = new SessionVault(context);
  private final File file = new File(context.getNoBackupFilesDir(), "session-v1.bin");

  private String token(char value) {
    char[] chars = new char[64];
    Arrays.fill(chars, value);
    return new String(chars);
  }

  @After
  public void cleanup() {
    vault.clear();
  }

  @Test
  public void ciphertext_is_random_authenticated_and_survives_recreation() throws Exception {
    String token = token('a');
    vault.save(token);
    byte[] first = Files.readAllBytes(file.toPath());
    assertEquals(101, first.length);
    assertFalse(new String(first, StandardCharsets.ISO_8859_1).contains(token));
    assertEquals(token, new SessionVault(context).load().token);
    vault.save(token);
    assertFalse(Arrays.equals(first, Files.readAllBytes(file.toPath())));
    assertEquals(token, vault.load().token);
    long time = System.currentTimeMillis() - 86400000L;
    vault.save(token, time);
    assertEquals(time, new SessionVault(context).load().rotatedAt);
    assertTrue(
        file.getCanonicalPath()
            .startsWith(context.getNoBackupFilesDir().getCanonicalPath() + File.separator));
  }

  @Test
  public void tampering_forces_login_and_clear_removes_all_ciphertext() throws Exception {
    vault.save(token('b'));
    byte[] bytes = Files.readAllBytes(file.toPath());
    bytes[32] ^= 1;
    try (FileOutputStream output = new FileOutputStream(file)) {
      output.write(bytes);
    }
    assertNull(vault.load());
    assertFalse(file.exists());
    vault.save(token('c'));
    vault.clear();
    assertNull(vault.load());
    assertFalse(file.exists());
  }

  @Test
  public void invalid_or_truncated_credentials_are_never_saved_or_returned() throws Exception {
    vault.clear();
    for (String invalid : new String[] {"", "not-a-token", token('g'), token('a') + "b"}) {
      try {
        vault.save(invalid);
        fail("invalid credential accepted");
      } catch (IllegalArgumentException expected) {
      }
    }
    assertFalse(file.exists());
    try (FileOutputStream output = new FileOutputStream(file)) {
      output.write(new byte[] {1, 2, 3});
    }
    assertNull(vault.load());
    assertFalse(file.exists());
  }
}
