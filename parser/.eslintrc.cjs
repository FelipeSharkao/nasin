/* eslint-env node */
module.exports = {
    env: {
        browser: true,
        es2021: true,
    },
    extends: ["eslint:recommended", "plugin:@typescript-eslint/recommended-type-checked"],
    parser: "@typescript-eslint/parser",
    plugins: ["@typescript-eslint"],
    root: true,
    parserOptions: {
        ecmaVersion: "latest",
        sourceType: "module",
        project: true,
    },
    rules: {
        "@typescript-eslint/require-await": "off",
    },
}
