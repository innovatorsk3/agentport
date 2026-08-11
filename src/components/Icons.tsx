// Inline SVG only — no icon package, no image files, nothing to bundle.

/** The wordmark glyph: two brackets with something crossing the gap between
 *  them. Leaving one machine, arriving at another — the product in one shape. */
export function Logo({ size = 30 }: { size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 32 32"
      fill="none"
      aria-hidden
      className="shrink-0"
    >
      <rect
        x="1"
        y="1"
        width="30"
        height="30"
        rx="8"
        className="fill-card stroke-border"
      />
      <path
        d="M11.5 9.5 L8 12.2 v7.6 l3.5 2.7"
        className="stroke-primary/75"
        strokeWidth="1.9"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <path
        d="M20.5 9.5 L24 12.2 v7.6 l-3.5 2.7"
        className="stroke-primary/75"
        strokeWidth="1.9"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <path
        d="M13.6 16 h5.2 m-2 -2.1 l2 2.1 l-2 2.1"
        className="stroke-foreground"
        strokeWidth="1.9"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

const s = {
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.6,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
};

type IconProps = { className?: string };

function Icon({
  children,
  className = "size-4",
}: IconProps & { children: React.ReactNode }) {
  return (
    <svg viewBox="0 0 20 20" className={className} aria-hidden>
      {children}
    </svg>
  );
}

export const ScanIcon = (p: IconProps) => (
  <Icon {...p}>
    <path {...s} d="M3 7V4.5A1.5 1.5 0 0 1 4.5 3H7" />
    <path {...s} d="M13 3h2.5A1.5 1.5 0 0 1 17 4.5V7" />
    <path {...s} d="M17 13v2.5a1.5 1.5 0 0 1-1.5 1.5H13" />
    <path {...s} d="M7 17H4.5A1.5 1.5 0 0 1 3 15.5V13" />
    <path {...s} d="M3.5 10h13" />
  </Icon>
);

export const ImportIcon = (p: IconProps) => (
  <Icon {...p}>
    <path {...s} d="M10 3v9" />
    <path {...s} d="M6.6 8.6 10 12l3.4-3.4" />
    <path {...s} d="M3.5 13.5v2A1.5 1.5 0 0 0 5 17h10a1.5 1.5 0 0 0 1.5-1.5v-2" />
  </Icon>
);

export const ExportIcon = (p: IconProps) => (
  <Icon {...p}>
    <path {...s} d="M10 12V3" />
    <path {...s} d="M6.6 6.4 10 3l3.4 3.4" />
    <path {...s} d="M3.5 13.5v2A1.5 1.5 0 0 0 5 17h10a1.5 1.5 0 0 0 1.5-1.5v-2" />
  </Icon>
);

export const PlusIcon = (p: IconProps) => (
  <Icon {...p}>
    <path {...s} d="M10 4.5v11M4.5 10h11" />
  </Icon>
);

export const CheckIcon = (p: IconProps) => (
  <Icon {...p}>
    <path {...s} d="M4.5 10.5 8 14l7.5-8" />
  </Icon>
);

export const AlertIcon = (p: IconProps) => (
  <Icon {...p}>
    <circle {...s} cx="10" cy="10" r="7" />
    <path {...s} d="M10 6.2v4.4M10 13.4v.2" />
  </Icon>
);

export const InfoIcon = (p: IconProps) => (
  <Icon {...p}>
    <circle {...s} cx="10" cy="10" r="7" />
    <path {...s} d="M10 9.2v4.4M10 6.4v.2" />
  </Icon>
);

export const BoltIcon = (p: IconProps) => (
  <Icon {...p}>
    <path {...s} d="M11 3 5 11h4l-1 6 6-8h-4l1-6Z" />
  </Icon>
);

export const TrashIcon = (p: IconProps) => (
  <Icon {...p}>
    <path {...s} d="M4 6h12" />
    <path {...s} d="M8 6V4.5A1.5 1.5 0 0 1 9.5 3h1A1.5 1.5 0 0 1 12 4.5V6" />
    <path {...s} d="M5.5 6l.6 9A1.5 1.5 0 0 0 7.6 16.5h4.8a1.5 1.5 0 0 0 1.5-1.5l.6-9" />
  </Icon>
);

export const PencilIcon = (p: IconProps) => (
  <Icon {...p}>
    <path {...s} d="M13.2 3.8a1.7 1.7 0 0 1 2.4 2.4L7.4 14.4 4 15l.6-3.4 8.6-7.8Z" />
  </Icon>
);

/** A spinner sized to sit inline with text. */
export const Spinner = ({ className = "size-4" }: IconProps) => (
  <svg viewBox="0 0 20 20" className={`animate-spin ${className}`} aria-hidden>
    <circle
      cx="10"
      cy="10"
      r="7"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      className="opacity-25"
    />
    <path
      d="M10 3a7 7 0 0 1 7 7"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
    />
  </svg>
);
