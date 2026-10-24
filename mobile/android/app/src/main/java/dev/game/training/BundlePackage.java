package dev.game.training;

import java.io.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.security.MessageDigest;
import java.util.*;
import java.util.zip.*;
import org.bouncycastle.crypto.params.Ed25519PublicKeyParameters;
import org.bouncycastle.crypto.signers.Ed25519Signer;

/** Signed, bounded web assets only. No downloaded native executable is ever accepted. */
final class BundlePackage {
  static final long MAX_ZIP = 10 * 1024 * 1024, MAX_EXPANDED = 40 * 1024 * 1024;

  static final class Manifest {
    final int version, minNative, protocol;
    final String content, sha256;
    final long size;
    final byte[] signature;

    Manifest(
        int version,
        int minNative,
        int protocol,
        String content,
        String sha256,
        long size,
        byte[] signature) {
      this.version = version;
      this.minNative = minNative;
      this.protocol = protocol;
      this.content = content;
      this.sha256 = sha256;
      this.size = size;
      this.signature = signature.clone();
    }

    byte[] payload() {
      return ("SPELLGAMBIT-WEB-1\n"
              + version
              + "\n"
              + minNative
              + "\n"
              + protocol
              + "\n"
              + content
              + "\n"
              + sha256
              + "\n"
              + size
              + "\n")
          .getBytes(StandardCharsets.US_ASCII);
    }

    void verify(byte[] key, int nativeCode, int currentProtocol, int highestVersion)
        throws IOException {
      if (version <= highestVersion
          || minNative < 1
          || minNative > nativeCode
          || protocol != currentProtocol
          || !content.matches("[a-f0-9]{64}")
          || !sha256.matches("[a-f0-9]{64}")
          || size < 1
          || size > MAX_ZIP
          || key.length != 32
          || signature.length != 64) throw new IOException("INCOMPATIBLE_BUNDLE");
      Ed25519Signer verifier = new Ed25519Signer();
      verifier.init(false, new Ed25519PublicKeyParameters(key, 0));
      byte[] payload = payload();
      verifier.update(payload, 0, payload.length);
      if (!verifier.verifySignature(signature)) throw new IOException("INVALID_SIGNATURE");
    }
  }

  static void verifyArchive(File archive, Manifest manifest) throws Exception {
    if (archive.length() != manifest.size) throw new IOException("INVALID_SIZE");
    MessageDigest hash = MessageDigest.getInstance("SHA-256");
    try (InputStream input = new FileInputStream(archive)) {
      byte[] bytes = new byte[8192];
      int n;
      while ((n = input.read(bytes)) != -1) hash.update(bytes, 0, n);
    }
    if (!hex(hash.digest()).equals(manifest.sha256)) throw new IOException("INVALID_HASH");
  }

  static void unpack(File archive, File target) throws IOException {
    if (target.exists() || !target.mkdirs()) throw new IOException("INVALID_TARGET");
    long total = 0;
    int count = 0;
    Set<String> names = new HashSet<>();
    try (ZipInputStream zip = new ZipInputStream(new FileInputStream(archive))) {
      ZipEntry entry;
      byte[] buffer = new byte[8192];
      while ((entry = zip.getNextEntry()) != null) {
        String name = entry.getName();
        if (++count > 5000
            || name.length() > 240
            || name.startsWith("/")
            || name.contains("\\")
            || name.contains(":")
            || name.contains("//")
            || Arrays.asList(name.split("/")).contains(".")
            || Arrays.asList(name.split("/", -1)).contains("..")
            || !names.add(name)) throw new IOException("UNSAFE_ARCHIVE");
        File file = new File(target, name);
        if (!file.getCanonicalPath().startsWith(target.getCanonicalPath() + File.separator))
          throw new IOException("UNSAFE_PATH");
        if (entry.isDirectory()) {
          if (!file.mkdirs() && !file.isDirectory()) throw new IOException("DIRECTORY_FAILED");
          continue;
        }
        if (!name.matches(
            "[A-Za-z0-9_./-]+\\.(html|js|mjs|css|wasm|json|svg|png|jpg|webp|avif|ico|woff2|txt|webmanifest)"))
          throw new IOException("UNSUPPORTED_ASSET");
        File parent = file.getParentFile();
        if (!parent.mkdirs() && !parent.isDirectory()) throw new IOException("DIRECTORY_FAILED");
        try (OutputStream out = new FileOutputStream(file)) {
          int n;
          while ((n = zip.read(buffer)) != -1) {
            total += n;
            if (total > MAX_EXPANDED) throw new IOException("ZIP_TOO_LARGE");
            out.write(buffer, 0, n);
          }
        }
      }
      if (!new File(target, "index.html").isFile()) throw new IOException("MISSING_ENTRYPOINT");
    } catch (IOException error) {
      delete(target);
      throw error;
    }
  }

  static void delete(File file) throws IOException {
    if (file.isDirectory()) {
      File[] children = file.listFiles();
      if (children == null) throw new IOException("UNREADABLE_DIRECTORY");
      for (File child : children) delete(child);
    }
    Files.deleteIfExists(file.toPath());
  }

  static String hex(byte[] bytes) {
    StringBuilder value = new StringBuilder();
    for (byte b : bytes) value.append(String.format(Locale.ROOT, "%02x", b & 255));
    return value.toString();
  }
}
