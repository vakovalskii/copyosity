/** @type {import('stylelint').Config} */
export default {
  extends: ["stylelint-config-standard", "stylelint-config-html/svelte"],
  plugins: ["stylelint-order", "stylelint-declaration-strict-value"],
  ignoreFiles: ["dist/**", "build/**", ".svelte-kit/**", "node_modules/**", "src-tauri/**"],
  rules: {
    // Oxfmt owns formatting and blank lines — avoid fighting the formatter.
    "at-rule-empty-line-before": null,
    "comment-empty-line-before": null,
    "custom-property-empty-line-before": null,
    "declaration-empty-line-before": null,
    "rule-empty-line-before": null,

    // macOS / WebKit surfaces (Tauri) still need targeted vendor prefixes.
    "property-no-vendor-prefix": [
      true,
      {
        ignoreProperties: [
          "appearance",
          "backdrop-filter",
          "-webkit-backdrop-filter",
          "box-orient",
          "line-clamp",
          "mask-image",
          "tap-highlight-color",
          "text-size-adjust",
          "user-select",
          "-webkit-user-select",
          "app-region",
        ],
      },
    ],
    "selector-no-vendor-prefix": [
      true,
      {
        ignoreSelectors: ["/-webkit-/"],
      },
    ],
    "value-no-vendor-prefix": [
      true,
      {
        ignoreValues: ["box", "inline-box", "vertical"],
      },
    ],

    // Allow standard + -webkit-* pairs (e.g. backdrop-filter for WKWebView/Tauri).
    "declaration-block-no-duplicate-properties": [
      true,
      {
        ignoreProperties: ["-webkit-backdrop-filter", "-webkit-user-select"],
      },
    ],

    // Interactive components intentionally order :hover / :focus-visible for cascade.
    "no-descending-specificity": null,

    // BEM-style modifiers (e.g. .form-section-title--with-icon) are used in shared form CSS.
    "selector-class-pattern": [
      "^([a-z][a-z0-9]*)(-[a-z0-9]+)*(--[a-z0-9]+(-[a-z0-9]+)*)?$",
      {
        message: (selector) => `Expected class selector "${selector}" to be kebab-case or BEM`,
      },
    ],
    // Svelte scoped styles
    "selector-pseudo-class-no-unknown": [
      true,
      {
        ignorePseudoClasses: ["global", "deep"],
      },
    ],

    "order/order": ["custom-properties", "declarations", "at-rules", "rules"],
  },
  overrides: [
    {
      files: ["src/lib/styles/tokens.css"],
      rules: {
        "color-hex-length": null,
        "number-max-precision": null,
        "font-family-no-missing-generic-family-keyword": null,
        "scale-unlimited/declaration-strict-value": null,
      },
    },
    {
      files: ["src/**/*.{css,svelte}"],
      excludedFiles: ["src/lib/styles/tokens.css"],
      rules: {
        "scale-unlimited/declaration-strict-value": [
          [
            "color",
            "background-color",
            "fill",
            "stroke",
            "caret-color",
            "outline-color",
            "text-decoration-color",
            "column-rule-color",
            "/^border(-.*)?-color$/",
          ],
          {
            ignoreValues: [
              "transparent",
              "currentColor",
              "currentcolor",
              "inherit",
              "initial",
              "unset",
              "revert",
              "revert-layer",
              "none",
              "/^var\\(--/",
            ],
            disableFix: true,
            expandShorthand: false,
          },
        ],
      },
    },
  ],
};
