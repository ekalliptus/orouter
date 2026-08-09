export default function Logo({ size = 36, className = "" }) {
  return (
    <span
      className={`inline-flex shrink-0 items-center justify-center ${className}`}
      style={{ width: size, height: size }}
      role="img"
      aria-label="ORouter"
    >
      <svg
        width={size}
        height={size}
        viewBox="0 0 32 32"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
        aria-hidden="true"
      >
        <rect x="4" y="4" width="25" height="25" fill="var(--nb-border, #000)" />
        <rect x="1" y="1" width="25" height="25" fill="#FF6B35" stroke="#000000" strokeWidth="2.5" />
        <circle cx="13.5" cy="13.5" r="8" fill="#FFD23F" stroke="#000000" strokeWidth="2.5" />
        <circle cx="13.5" cy="13.5" r="3.5" fill="#FF6B35" stroke="#000000" strokeWidth="2" />
      </svg>
    </span>
  );
}
