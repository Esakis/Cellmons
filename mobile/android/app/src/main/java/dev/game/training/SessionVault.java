package dev.game.training;

import android.content.Context;
import android.security.keystore.KeyGenParameterSpec;
import android.security.keystore.KeyProperties;
import android.util.AtomicFile;
import java.io.File;
import java.io.FileOutputStream;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.security.KeyStore;
import java.util.Arrays;
import javax.crypto.Cipher;
import javax.crypto.KeyGenerator;
import javax.crypto.SecretKey;
import javax.crypto.spec.GCMParameterSpec;

/**
 * Only ciphertext enters disk. The key is non-exportable and the file is outside Android backups.
 */
final class SessionVault {
  static final class Stored {
    final String token;
    final long rotatedAt;

    Stored(String token, long rotatedAt) {
      this.token = token;
      this.rotatedAt = rotatedAt;
    }
  }

  private final AtomicFile file;
  private final String alias;
  private final byte[] aad;

  SessionVault(Context context) {
    file = new AtomicFile(new File(context.getNoBackupFilesDir(), "session-v1.bin"));
    alias = context.getPackageName() + ".session.v1";
    aad = (alias + ":AES256GCM").getBytes(StandardCharsets.UTF_8);
  }

  private SecretKey key(boolean create) throws Exception {
    KeyStore keys = KeyStore.getInstance("AndroidKeyStore");
    keys.load(null);
    SecretKey stored = (SecretKey) keys.getKey(alias, null);
    if (stored != null || !create) return stored;
    KeyGenerator generator =
        KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore");
    generator.init(
        new KeyGenParameterSpec.Builder(
                alias, KeyProperties.PURPOSE_ENCRYPT | KeyProperties.PURPOSE_DECRYPT)
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setKeySize(256)
            .setRandomizedEncryptionRequired(true)
            .build());
    return generator.generateKey();
  }

  synchronized void save(String token) throws Exception {
    save(token, System.currentTimeMillis());
  }

  synchronized void save(String token, long rotatedAt) throws Exception {
    if (token == null || !token.matches("[0-9a-f]{64}"))
      throw new IllegalArgumentException("INVALID_TOKEN");
    if (rotatedAt <= 0) throw new IllegalArgumentException("INVALID_TIMESTAMP");
    Cipher cipher = Cipher.getInstance("AES/GCM/NoPadding");
    cipher.init(Cipher.ENCRYPT_MODE, key(true));
    cipher.updateAAD(aad);
    byte[] plain =
        ByteBuffer.allocate(72)
            .putLong(rotatedAt)
            .put(token.getBytes(StandardCharsets.US_ASCII))
            .array();
    byte[] encrypted;
    try {
      encrypted = cipher.doFinal(plain);
    } finally {
      Arrays.fill(plain, (byte) 0);
    }
    if (cipher.getIV().length != 12) throw new IllegalStateException("INVALID_IV");
    FileOutputStream output = null;
    try {
      output = file.startWrite();
      output.write(1);
      output.write(cipher.getIV());
      output.write(encrypted);
      file.finishWrite(output);
    } catch (Exception error) {
      if (output != null) file.failWrite(output);
      throw error;
    }
  }

  synchronized Stored load() {
    if (!file.getBaseFile().exists()) return null;
    try {
      if (file.getBaseFile().length() != 101) throw new IllegalStateException("INVALID_CIPHERTEXT");
      byte[] bytes = file.readFully();
      if (bytes[0] != 1) throw new IllegalStateException("INVALID_VERSION");
      SecretKey stored = key(false);
      if (stored == null) throw new IllegalStateException("KEY_UNAVAILABLE");
      Cipher cipher = Cipher.getInstance("AES/GCM/NoPadding");
      cipher.init(
          Cipher.DECRYPT_MODE, stored, new GCMParameterSpec(128, Arrays.copyOfRange(bytes, 1, 13)));
      cipher.updateAAD(aad);
      byte[] plain = cipher.doFinal(Arrays.copyOfRange(bytes, 13, bytes.length));
      try {
        long rotatedAt = ByteBuffer.wrap(plain, 0, 8).getLong();
        String token = new String(plain, 8, 64, StandardCharsets.US_ASCII);
        if (!token.matches("[0-9a-f]{64}")) throw new IllegalStateException("INVALID_TOKEN");
        if (rotatedAt <= 0) throw new IllegalStateException("INVALID_TIMESTAMP");
        return new Stored(token, rotatedAt);
      } finally {
        Arrays.fill(plain, (byte) 0);
      }
    } catch (Exception unavailable) {
      // A missing/invalidated key or modified file requires a new login. Never log the token.
      file.delete();
      return null;
    }
  }

  synchronized void clear() {
    file.delete();
  }
}
