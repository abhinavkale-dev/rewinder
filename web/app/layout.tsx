import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "Rewinder — never miss the moment",
  description:
    "Rewinder keeps a rolling replay of your Mac. Press a hotkey, save the last few minutes as a clip.",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
