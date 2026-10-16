package dev.game.training;

/** Pure state machine: a failed trial always boots its previous confirmed release. */
final class BundleState {
  int current, previous, pending, highest;
  boolean trial;

  BundleState(int current, int previous, int pending, int highest, boolean trial) {
    this.current = current;
    this.previous = previous;
    this.pending = pending;
    this.highest = highest;
    this.trial = trial;
  }

  void boot() {
    if (trial) {
      current = previous;
      trial = false;
      pending = 0;
    }
  }

  boolean apply(boolean idle) {
    if (!idle || trial || pending <= highest) return false;
    previous = current;
    current = pending;
    highest = current;
    pending = 0;
    trial = true;
    return true;
  }

  void confirm() {
    if (trial) {
      highest = Math.max(highest, current);
      trial = false;
    }
  }
}
