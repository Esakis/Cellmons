package dev.game.training;

import static org.junit.Assert.*;

import java.io.*;
import java.nio.file.*;
import java.security.*;
import java.util.zip.*;
import org.bouncycastle.crypto.params.Ed25519PrivateKeyParameters;
import org.bouncycastle.crypto.signers.Ed25519Signer;
import org.junit.Test;

public class BundlePackageTest {
  private final Ed25519PrivateKeyParameters key =
      new Ed25519PrivateKeyParameters(new SecureRandom());

  private BundlePackage.Manifest signed(int version, int min, int protocol) {
    BundlePackage.Manifest unsigned =
        new BundlePackage.Manifest(
            version, min, protocol, "a".repeat(64), "b".repeat(64), 100, new byte[64]);
    Ed25519Signer signer = new Ed25519Signer();
    signer.init(true, key);
    byte[] payload = unsigned.payload();
    signer.update(payload, 0, payload.length);
    return new BundlePackage.Manifest(
        version,
        min,
        protocol,
        unsigned.content,
        unsigned.sha256,
        unsigned.size,
        signer.generateSignature());
  }

  @Test
  public void authenticSignatureAndIncompatibleReleases() throws Exception {
    BundlePackage.Manifest good = signed(2, 10000, 1);
    byte[] publicKey = key.generatePublicKey().getEncoded();
    good.verify(publicKey, 10000, 1, 1);
    assertThrows(IOException.class, () -> good.verify(publicKey, 9999, 1, 1));
    assertThrows(IOException.class, () -> good.verify(publicKey, 10000, 2, 1));
    assertThrows(IOException.class, () -> good.verify(publicKey, 10000, 1, 2));
    good.signature[0] ^= 1;
    assertThrows(IOException.class, () -> good.verify(publicKey, 10000, 1, 1));
  }

  @Test
  public void alteredManifestIsNotAuthenticated() {
    BundlePackage.Manifest good = signed(2, 10000, 1);
    BundlePackage.Manifest altered =
        new BundlePackage.Manifest(3, 10000, 1, good.content, good.sha256, 100, good.signature);
    assertThrows(
        IOException.class, () -> altered.verify(key.generatePublicKey().getEncoded(), 10000, 1, 1));
  }

  private File archive(String name) throws Exception {
    File archive = File.createTempFile("bundle-test-", ".zip");
    archive.deleteOnExit();
    try (ZipOutputStream zip = new ZipOutputStream(new FileOutputStream(archive))) {
      zip.putNextEntry(new ZipEntry(name));
      zip.write("ok".getBytes());
      zip.closeEntry();
    }
    return archive;
  }

  @Test
  public void safeArchiveAndHash() throws Exception {
    File archive = archive("index.html");
    File parent = Files.createTempDirectory("web-package-").toFile();
    try {
      String hash =
          BundlePackage.hex(
              MessageDigest.getInstance("SHA-256").digest(Files.readAllBytes(archive.toPath())));
      BundlePackage.Manifest manifest =
          new BundlePackage.Manifest(
              2, 10000, 1, "a".repeat(64), hash, archive.length(), new byte[64]);
      BundlePackage.verifyArchive(archive, manifest);
      BundlePackage.unpack(archive, new File(parent, "safe"));
      assertEquals(
          "ok",
          new String(
              Files.readAllBytes(new File(parent, "safe/index.html").toPath()),
              java.nio.charset.StandardCharsets.UTF_8));
      Files.write(archive.toPath(), new byte[] {1, 2, 3});
      assertThrows(IOException.class, () -> BundlePackage.verifyArchive(archive, manifest));
    } finally {
      BundlePackage.delete(parent);
      BundlePackage.delete(archive);
    }
  }

  @Test
  public void paintedAvifAssetsAreAccepted() throws Exception {
    File parent = Files.createTempDirectory("web-package-avif-").toFile();
    File archive = new File(parent, "bundle.zip");
    try {
      try (ZipOutputStream zip = new ZipOutputStream(new FileOutputStream(archive))) {
        for (String name : new String[] {"index.html", "assets/town.avif"}) {
          zip.putNextEntry(new ZipEntry(name));
          zip.write("painted asset".getBytes(java.nio.charset.StandardCharsets.UTF_8));
          zip.closeEntry();
        }
      }
      File target = new File(parent, "unpacked");
      BundlePackage.unpack(archive, target);
      assertEquals(
          "painted asset",
          new String(
              Files.readAllBytes(new File(target, "assets/town.avif").toPath()),
              java.nio.charset.StandardCharsets.UTF_8));
    } finally {
      BundlePackage.delete(parent);
    }
  }

  @Test
  public void traversalAndNativeCodeAreRejectedAndCleaned() throws Exception {
    File parent = Files.createTempDirectory("web-package-").toFile();
    try {
      for (String name :
          new String[] {
            "../escape.js",
            "/escape.js",
            "assets/evil.dex",
            "assets/evil.so",
            "assets\\escape.js",
            "assets//x.js",
            "assets/./x.js"
          }) {
        File archive = archive(name), target = new File(parent, "unsafe");
        assertThrows(IOException.class, () -> BundlePackage.unpack(archive, target));
        assertFalse(target.exists());
        BundlePackage.delete(archive);
      }
    } finally {
      BundlePackage.delete(parent);
    }
  }

  @Test
  public void busyTrialCrashAndWatchdogRollback() {
    BundleState state = new BundleState(1, 0, 2, 1, false);
    assertFalse(state.apply(false));
    assertEquals(1, state.current);
    assertEquals(2, state.pending);
    assertTrue(state.apply(true));
    assertTrue(state.trial);
    assertEquals(2, state.current);
    state.boot();
    assertEquals(1, state.current);
    assertFalse(state.trial);
    assertEquals(2, state.highest);
    state.pending = 2;
    assertFalse(state.apply(true));
  }

  @Test
  public void healthyBootIsRetainedAndRollbackHasAConfirmedPrevious() {
    BundleState state = new BundleState(1, 0, 2, 1, false);
    assertTrue(state.apply(true));
    state.confirm();
    state.boot();
    assertEquals(2, state.current);
    state.pending = 3;
    assertTrue(state.apply(true));
    state.boot();
    assertEquals(2, state.current);
  }
}
