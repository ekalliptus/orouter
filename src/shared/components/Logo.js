"use client";

/** Leaf mark: one quiet identity shared by navigation and sign-in. */
export default function Logo({ size = 36, className = "" }) {
  return (
    <span className={`inline-flex shrink-0 items-center justify-center text-primary ${className}`} style={{ width: size, height: size }} role="img" aria-label="ORouter">
      <svg width={size} height={size} viewBox="0 0 32 32" fill="none" aria-hidden="true">
        <path d="M27 5C16 3 5 7 5 17c0 6 5 10 11 9C26 24 28 14 27 5Z" fill="currentColor" />
        <path d="M9 25 23 10M14 20v-7m0 7h7" stroke="var(--color-surface)" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" />
      </svg>
    </span>
  );
}
