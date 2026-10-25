import { useRef } from "react";
import { useScene } from "./useScene";
import type { Beat } from "./useScene";
import "./Mock.css";

const ICON = {
  width: 16,
  height: 16,
  viewBox: "0 0 16 16",
  "aria-hidden": true,
} as const;

/** What the window draws over a part of the page while the person comments: their hover, the
 *  card for their note, then a numbered mark (probe/feedback.js, as it looks). */
function Target({
  name,
  beat,
  mark,
  children,
}: {
  name: "button" | "total";
  beat: Beat;
  mark: number;
  children: React.ReactNode;
}) {
  const hovered = beat.target === name;
  const marked = beat.marks >= mark;
  return (
    <span
      className={`mk-target${hovered ? " hover" : ""}${marked ? " marked" : ""}`}
    >
      {children}
      {marked && <b className="mk-badge">{mark}</b>}
      {hovered && beat.card && (
        <span className="mk-card">
          <span className="mk-note">
            {beat.note || (
              <span className="mk-ph">What should change? (optional)</span>
            )}
            <i className="mk-caret" />
          </span>
          <span className="mk-actions">
            <span>Cancel</span>
            <span className="send">Send</span>
          </span>
        </span>
      )}
    </span>
  );
}

/** A tab as the strip draws it: its kind's icon, its title, and × on the one shown. */
function Tab({
  kind,
  name,
  on,
}: {
  kind: "page" | "term";
  name: string;
  on: boolean;
}) {
  return (
    <span className={`mk-t${on ? " on" : ""}`}>
      <span className="mk-i">{kind === "term" ? ">_" : "◐"}</span>
      <span className="mk-n">{name}</span>
      <span className="mk-x">×</span>
    </span>
  );
}

function Flights({ beat }: { beat: Beat }) {
  const rows = [
    ["SWISS", "07:00", "07:50", "CHF 89"],
    ["easyJet", "09:15", "10:05", "CHF 64"],
    ["British Airways", "12:40", "13:35", "CHF 112"],
  ];
  return (
    <div className="mk-view mk-paper">
      <div className="mk-search">
        <span className="mk-field">
          {beat.from || <span className="mk-ph">From</span>}
        </span>
        <span className="mk-arrow">→</span>
        <span className="mk-field">
          {beat.to || <span className="mk-ph">To</span>}
        </span>
        <span className="mk-field date">Fri 30 Oct</span>
      </div>
      <div className="mk-rows">
        {beat.results
          ? rows.map(([airline, dep, arr, price]) => {
              const picked = beat.picked && airline === "easyJet";
              return (
                <div
                  key={airline}
                  className={`mk-row${picked ? " picked" : ""}`}
                >
                  <i className="mk-logo" />
                  <b>{airline}</b>
                  <span className="mk-time">
                    {dep} <i className="mk-line" /> {arr}
                  </span>
                  <span className="mk-price">{price}</span>
                  <span
                    className={`mk-pill${picked && beat.booked ? " done" : ""}`}
                  >
                    {picked && beat.booked ? "Booked ✓" : "Select"}
                  </span>
                </div>
              );
            })
          : [0, 1, 2].map((n) => <div key={n} className="mk-row skeleton" />)}
      </div>
    </div>
  );
}

const TEST_LINES = [
  "$ npm test",
  "  ✓ cart adds an item",
  "  ✓ total includes shipping",
  "  ✓ checkout pays",
  "  24 passed · 1.2 s",
];

function Terminal({ beat }: { beat: Beat }) {
  return (
    <div className="mk-view mk-term">
      {TEST_LINES.slice(0, beat.lines).map((line, i) => (
        <span key={line} className={i > 0 ? "ok" : undefined}>
          {line}
          {i === beat.lines - 1 && <i className="mk-caret" />}
        </span>
      ))}
    </div>
  );
}

function Checkout({ beat }: { beat: Beat }) {
  return (
    <div
      className={`mk-view mk-paper mk-page${beat.commenting ? " commenting" : ""}`}
    >
      <div className="mk-head">Checkout</div>
      <div className="mk-item">
        <i className="mk-thumb" />
        <span>
          Trail runners
          <i className="mk-bone" />
        </span>
        <span>€39.00</span>
      </div>
      <Target name="total" beat={beat} mark={2}>
        <span className="mk-total">Total €42.00</span>
      </Target>
      <Target name="button" beat={beat} mark={1}>
        <span className="mk-button">Pay now</span>
      </Target>
    </div>
  );
}

/** The llx window as it is drawn, playing a short scene: projects on top with their tabs under
 *  them; the agent searches flights in one project, runs the tests in a terminal of the other,
 *  and the person comments on its page and sends. */
export function Mock() {
  const root = useRef<HTMLDivElement>(null);
  const beat = useScene(root);
  const shop = beat.project === "shop";
  return (
    <div ref={root} className="mk-wrap">
      <figure
        className="mk"
        aria-label="The llx window: projects with their tabs, the agent in a browser and a terminal, the person commenting"
      >
        <div className="mk-strip">
          <div className="mk-projects">
            <span className="mk-lights" aria-hidden="true">
              <i />
              <i />
              <i />
            </span>
            <span className={`mk-p${shop ? "" : " on"}`}>trips</span>
            <span className={`mk-p${shop ? " on" : ""}`}>shop</span>
          </div>
          <div className="mk-bar">
            <span className="mk-b">
              <svg {...ICON}>
                <path d="M10 3 5 8l5 5" />
              </svg>
            </span>
            <span className="mk-b">
              <svg {...ICON}>
                <path d="m6 3 5 5-5 5" />
              </svg>
            </span>
            {shop ? (
              <>
                <Tab kind="term" name="npm test" on={beat.tab === "term"} />
                <Tab kind="page" name="Checkout" on={beat.tab === "page"} />
              </>
            ) : (
              <>
                <Tab kind="page" name="Flights Zürich → London" on />
                <Tab kind="page" name="Hotels in London" on={false} />
              </>
            )}
            <span className="mk-gap" />
            {beat.marks > 0 && <span className="mk-s">Send {beat.marks}</span>}
            <span className={`mk-b${beat.commenting ? " cm" : ""}`}>
              <svg {...ICON}>
                <path d="M3 3.5h10v7H7l-3 2.5v-2.5H3z" />
              </svg>
            </span>
            <span className="mk-b">
              <svg {...ICON}>
                <circle cx="8" cy="8" r="6" />
                <path d="M8 4.5V8l2.3 1.4" />
              </svg>
            </span>
          </div>
        </div>

        {beat.tab === "flights" && <Flights beat={beat} />}
        {beat.tab === "term" && <Terminal beat={beat} />}
        {beat.tab === "page" && <Checkout beat={beat} />}
      </figure>
    </div>
  );
}
