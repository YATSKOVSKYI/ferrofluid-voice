/** @type {import('tailwindcss').Config} */
export default {
  darkMode: "class",
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      fontFamily: {
        sans: [
          "-apple-system",
          "BlinkMacSystemFont",
          "SF Pro Display",
          "Segoe UI",
          "Inter",
          "system-ui",
          "sans-serif",
        ],
      },
      boxShadow: {
        glass: "0 24px 80px rgba(15, 23, 42, 0.18)",
        insetGlass: "inset 0 1px 0 rgba(255, 255, 255, 0.55)",
      },
    },
  },
  plugins: [],
};
