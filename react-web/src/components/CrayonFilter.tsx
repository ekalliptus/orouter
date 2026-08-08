// One injected SVG that defines the crayon-edge displacement filters used by
// kiddraw.css (.kid-wobble / .kid-wobble-strong). Rendered once near the app
// root; it paints nothing, it just hosts the <filter> definitions referenced
// by `filter: url(#...)`.
//
// feTurbulence generates noise; feDisplacementMap pushes the element's edge
// along that noise, so straight borders wobble like a hand-drawn line.
// `--wobble` (theme.css, default 2.4) scales the displacement via the scale
// attribute. Two strengths are exposed.
export default function CrayonFilter() {
  return (
    <svg
      aria-hidden="true"
      style={{ position: "absolute", width: 0, height: 0, pointerEvents: "none" }}
    >
      <defs>
        <filter id="crayon-edge" x="-3%" y="-3%" width="106%" height="106%">
          <feTurbulence type="fractalNoise" baseFrequency="0.018 0.022" numOctaves={2} seed={7} result="noise" />
          <feDisplacementMap in="SourceGraphic" in2="noise" scale={2.4} xChannelSelector="R" yChannelSelector="G" />
        </filter>
        <filter id="crayon-edge-strong" x="-5%" y="-5%" width="110%" height="110%">
          <feTurbulence type="fractalNoise" baseFrequency="0.012 0.016" numOctaves={3} seed={11} result="noise" />
          <feDisplacementMap in="SourceGraphic" in2="noise" scale={4.5} xChannelSelector="R" yChannelSelector="G" />
        </filter>
      </defs>
    </svg>
  );
}
