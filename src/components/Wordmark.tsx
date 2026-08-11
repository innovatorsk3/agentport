import { Logo } from "./Icons";

/** The wordmark: all-caps with wide tracking.
 *
 *  Caps were tested against five other treatments side by side. Lowercase read
 *  as a half-finished dev script no matter how it was weighted or tracked; caps
 *  stop shouting once the tracking opens up, and — unlike lowercase — the two
 *  halves share a cap height, so the colour change lands on a clean seam rather
 *  than a visual break. */
export function Wordmark({ compact = false }: { compact?: boolean }) {
  return (
    <div className="flex items-center gap-3">
      <Logo size={compact ? 26 : 30} />
      <span
        className={`font-semibold tracking-[0.16em] ${
          compact ? "text-[13px]" : "text-[15px]"
        }`}
      >
        AGENT<span className="text-primary">PORT</span>
      </span>
    </div>
  );
}
