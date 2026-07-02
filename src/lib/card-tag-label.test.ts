import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  CARD_TAG_NEVER_TRUNCATE_MAX_LEN,
  CARD_TAG_THREE_LABEL_BUDGET,
  CARD_TAG_TRUNCATE_LABEL_MIN,
  cardTagDbVariants,
  cardTagFooterTruncateFlags,
  cardTagShouldTruncate,
  cardTagThreeTruncateFlags,
  cardTagTitle,
  cardTagsShareDisplayLabel,
  formatCardTagLabel,
} from "./card-tag-label.ts";

describe("card tag constants", () => {
  it("keeps truncation policy thresholds stable", () => {
    assert.equal(CARD_TAG_THREE_LABEL_BUDGET, 22);
    assert.equal(CARD_TAG_NEVER_TRUNCATE_MAX_LEN, 7);
    assert.equal(CARD_TAG_TRUNCATE_LABEL_MIN, 12);
  });
});

describe("formatCardTagLabel", () => {
  it("abbreviates widely recognized long tags", () => {
    assert.equal(formatCardTagLabel("kubernetes"), "k8s");
    assert.equal(formatCardTagLabel("javascript"), "js");
    assert.equal(formatCardTagLabel("typescript"), "ts");
  });

  it("leaves short or unknown tags unchanged", () => {
    assert.equal(formatCardTagLabel("docker"), "docker");
    assert.equal(formatCardTagLabel("api"), "api");
    assert.equal(formatCardTagLabel("credentials"), "credentials");
    assert.equal(formatCardTagLabel("postgresql"), "postgresql");
    assert.equal(formatCardTagLabel("elasticsearch"), "elasticsearch");
    assert.equal(formatCardTagLabel("terraform"), "terraform");
  });

  it("matches case-insensitively", () => {
    assert.equal(formatCardTagLabel("Kubernetes"), "k8s");
  });
});

describe("cardTagDbVariants", () => {
  it("groups javascript and js under js", () => {
    assert.deepEqual(cardTagDbVariants("js"), ["javascript", "js"]);
  });

  it("returns identity for unmapped tags", () => {
    assert.deepEqual(cardTagDbVariants("docker"), ["docker"]);
  });
});

describe("cardTagsShareDisplayLabel", () => {
  it("treats synonym tags as equivalent", () => {
    assert.equal(cardTagsShareDisplayLabel("javascript", "js"), true);
    assert.equal(cardTagsShareDisplayLabel("docker", "kubernetes"), false);
  });
});

describe("cardTagShouldTruncate", () => {
  it("does not truncate short labels", () => {
    assert.equal(cardTagShouldTruncate("api"), false);
    assert.equal(cardTagShouldTruncate("go"), false);
    assert.equal(cardTagShouldTruncate("javascript"), false);
  });

  it("truncates long labels", () => {
    assert.equal(cardTagShouldTruncate("distributed-systems-orchestration-layer"), true);
    assert.equal(cardTagShouldTruncate("enterprise-architecture-platform-v2"), true);
  });

  it("uses a strict greater-than threshold at the min length", () => {
    const atMin = "x".repeat(CARD_TAG_TRUNCATE_LABEL_MIN);
    const aboveMin = "x".repeat(CARD_TAG_TRUNCATE_LABEL_MIN + 1);
    assert.equal(cardTagShouldTruncate(atMin), false);
    assert.equal(cardTagShouldTruncate(aboveMin), true);
  });
});

describe("cardTagThreeTruncateFlags", () => {
  it("fits three seven-char labels within the budget", () => {
    const labels = ["1234567", "1234567", "1234567"];
    assert.equal(labels.join("").length, 21);
    assert.deepEqual(cardTagThreeTruncateFlags(labels), [false, false, false]);
  });

  it("truncates one eight-char label when three eights exceed the budget", () => {
    const labels = ["12345678", "12345678", "12345678"];
    assert.equal(labels.join("").length, 24);
    const flags = cardTagThreeTruncateFlags(labels);
    assert.deepEqual(flags, [true, false, false]);
  });

  it("never truncates labels at or below the short-tag limit", () => {
    const labels = ["1234567", "123456789012345", "123456789012345"];
    const flags = cardTagThreeTruncateFlags(labels);
    assert.equal(flags[0], false);
    assert.equal(flags[1], true);
    assert.equal(flags[2], true);
  });

  it("truncates tied longest labels together (10 + 10 + 7)", () => {
    const labels = ["1234567890", "1234567890", "1234567"];
    assert.equal(labels.join("").length, 27);
    assert.deepEqual(cardTagThreeTruncateFlags(labels), [true, true, false]);
  });

  it("truncates longest then second longest (10 + 9 + 8)", () => {
    const labels = ["1234567890", "123456789", "12345678"];
    assert.deepEqual(cardTagThreeTruncateFlags(labels), [true, true, false]);
  });

  it("marks all three twelve-char labels for ellipsis when they exceed the budget", () => {
    const labels = ["123456789012", "123456789012", "123456789012"];
    assert.equal(labels.join("").length, 36);
    const flags = cardTagThreeTruncateFlags(labels);
    assert.equal(flags.filter(Boolean).length, 3);
    assert.equal(flags[0], true);
  });

  it("matches TagMock-9 style rows (long + long + api)", () => {
    const labels = [
      formatCardTagLabel("synchronization"),
      formatCardTagLabel("containerization"),
      formatCardTagLabel("api"),
    ];
    const flags = cardTagThreeTruncateFlags(labels);
    assert.equal(flags[2], false);
    assert.equal(flags.filter(Boolean).length, 2);
  });
});

describe("cardTagFooterTruncateFlags", () => {
  it("uses the three-tag budget for three labels", () => {
    assert.deepEqual(cardTagFooterTruncateFlags(["1234567", "1234567", "1234567"]), [
      false,
      false,
      false,
    ]);
  });

  it("uses per-label threshold for one or two labels", () => {
    assert.deepEqual(
      cardTagFooterTruncateFlags(["api", "distributed-systems-orchestration-layer"]),
      [false, true],
    );
  });
});

describe("cardTagTitle", () => {
  it("returns the full tag when abbreviated", () => {
    assert.equal(cardTagTitle("kubernetes", formatCardTagLabel("kubernetes"), false), "kubernetes");
  });

  it("returns the full tag when truncation may apply", () => {
    assert.equal(cardTagTitle("api", "api", true), "api");
    assert.equal(
      cardTagTitle(
        "enterprise-architecture-platform-v2",
        "enterprise-architecture-platform-v2",
        true,
      ),
      "enterprise-architecture-platform-v2",
    );
  });

  it("omits title when the label is shown in full without clipping risk", () => {
    assert.equal(cardTagTitle("api", "api", false), undefined);
    assert.equal(cardTagTitle("docker", "docker", false), undefined);
  });
});
