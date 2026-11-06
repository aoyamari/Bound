import { useCallback, useEffect, useState } from 'react'
import {
  CONTRACT_ID,
  EXPLORER_URL,
  canSpend,
  formatNear,
  getPolicy,
  getSpentToday,
  nearToYocto,
  yoctoToNear,
  type CanSpendView,
  type PolicyView,
} from './near'

type Focus = 'architecture' | 'simulate'

type Scenario = {
  id: string
  title: string
  receiver: string
  amountNear: string
  blurb: string
}

const SCENARIOS: Scenario[] = [
  {
    id: 'ok',
    title: 'Allowed payment',
    receiver: 'vendor.bound-aoyamari.testnet',
    amountNear: '0.1',
    blurb: 'whitelisted · under max',
  },
  {
    id: 'over',
    title: 'Over max / tx',
    receiver: 'vendor.bound-aoyamari.testnet',
    amountNear: '0.3',
    blurb: '0.3 Ⓝ compared with live max_per_tx',
  },
  {
    id: 'unlisted',
    title: 'Unlisted payee',
    receiver: 'near-examples.testnet',
    amountNear: '0.1',
    blurb: 'not on whitelist',
  },
]

function scenarioBlurb(scenario: Scenario, policy: PolicyView | null): string {
  if (scenario.id !== 'over') return scenario.blurb
  if (!policy) return scenario.blurb
  const probe = BigInt(nearToYocto(scenario.amountNear))
  const max = BigInt(policy.max_per_tx)
  const relation = probe > max ? '>' : probe === max ? '=' : '<'
  return `${scenario.amountNear} Ⓝ ${relation} live max_per_tx ${yoctoToNear(policy.max_per_tx, 4)} Ⓝ`
}

type SimResult = {
  scenario: Scenario
  view: CanSpendView
  at: string
}

export default function App() {
  const [policy, setPolicy] = useState<PolicyView | null>(null)
  const [spentToday, setSpentToday] = useState<string | null>(null)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)
  const [focus, setFocus] = useState<Focus>('simulate')
  const [simBusy, setSimBusy] = useState(false)
  const [simError, setSimError] = useState<string | null>(null)
  const [result, setResult] = useState<SimResult | null>(null)
  const [log, setLog] = useState<SimResult[]>([])

  const refresh = useCallback(async () => {
    setLoadError(null)
    try {
      const [p, spent] = await Promise.all([getPolicy(), getSpentToday()])
      setPolicy(p)
      setSpentToday(typeof spent === 'string' ? spent : String(spent))
    } catch (e) {
      setLoadError(e instanceof Error ? e.message : String(e))
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void refresh()
    const id = window.setInterval(() => void refresh(), 30_000)
    return () => window.clearInterval(id)
  }, [refresh])

  const runScenario = async (scenario: Scenario) => {
    setSimBusy(true)
    setSimError(null)
    try {
      const amount = nearToYocto(scenario.amountNear)
      const view = await canSpend(scenario.receiver, amount)
      const entry: SimResult = {
        scenario,
        view,
        at: new Date().toLocaleTimeString('en-GB', { hour12: false }),
      }
      setResult(entry)
      setLog((prev) => [entry, ...prev].slice(0, 8))
      // Refresh spent_today after dry-run (read-only; values unchanged, but keeps UI honest)
      void refresh()
    } catch (e) {
      setSimError(e instanceof Error ? e.message : String(e))
    } finally {
      setSimBusy(false)
    }
  }

  return (
    <div className="page">
      <header className="topbar">
        <div className="brand">
          <span className="brand-mark">Bound</span>
          <span className="brand-sub">on-chain agent guardian</span>
        </div>
        <div className="badge-row">
          <span className="badge live">
            <span className="dot" aria-hidden />
            LIVE NEAR TESTNET
          </span>
          <a className="badge mono" href={EXPLORER_URL} target="_blank" rel="noreferrer">
            {CONTRACT_ID}
          </a>
          <button className="refresh" type="button" onClick={() => void refresh()}>
            Refresh
          </button>
        </div>
      </header>

      <section className="hero">
        <div>
          <h1>
            Autonomy,
            <br />
            <em>within bounds.</em>
          </h1>
          <p className="tagline">
            Public spending policy for AI agents. Limits and whitelist live on NEAR — verifiable by anyone.
          </p>
        </div>
        <div className="hero-meta">
          <strong>Read-only demo</strong>
          Live <code className="mono">get_policy</code>, <code className="mono">get_spent_today</code>, and{' '}
          <code className="mono">can_spend</code> via public RPC. No wallet. No keys in the browser.
        </div>
      </section>

      {loadError && (
        <div className="error-banner" role="alert">
          Could not load live policy: {loadError}
        </div>
      )}

      <section className="stats" aria-label="Live policy stats">
        <article className="stat">
          <label>Max per payment</label>
          <div className="value">
            {policy ? formatNear(policy.max_per_tx) : loading ? '…' : '—'}
          </div>
          <div className="hint">max_per_tx</div>
        </article>
        <article className="stat">
          <label>Daily allowance</label>
          <div className="value">
            {policy ? formatNear(policy.daily_limit) : loading ? '…' : '—'}
          </div>
          <div className="hint">daily_limit</div>
        </article>
        <article className="stat">
          <label>Spent today</label>
          <div className="value">
            {spentToday != null ? formatNear(spentToday) : loading ? '…' : '—'}
          </div>
          <div className="hint">get_spent_today</div>
        </article>
      </section>

      <section className="panel">
        <div className="panel-head">
          <h2>Guardian focus</h2>
          <div className="toggle" role="tablist" aria-label="Focus">
            <button
              type="button"
              className={focus === 'simulate' ? 'active' : ''}
              onClick={() => setFocus('simulate')}
            >
              Live simulate
            </button>
            <button
              type="button"
              className={focus === 'architecture' ? 'active' : ''}
              onClick={() => setFocus('architecture')}
            >
              Architecture
            </button>
          </div>
        </div>

        {focus === 'simulate' ? (
          <>
            <p className="idle-hint" style={{ marginTop: 0 }}>
              Three dry-runs call <code className="mono">can_spend</code> on{' '}
              <a href={EXPLORER_URL} target="_blank" rel="noreferrer">
                {CONTRACT_ID}
              </a>
              . Nothing is signed or transferred.
            </p>
            {policy?.paused && (
              <p className="idle-hint">
                Guardian is paused. <code className="mono">can_spend</code> returns{' '}
                <code className="mono">paused</code> before whitelist, caps, and balance. Owner
                withdraw stays available.
              </p>
            )}
            <div className="scenarios">
              {SCENARIOS.map((s) => (
                <button
                  key={s.id}
                  type="button"
                  className="scenario"
                  disabled={simBusy}
                  onClick={() => void runScenario(s)}
                >
                  <div className="title">{s.title}</div>
                  <div className="detail">
                    {s.amountNear} Ⓝ → {s.receiver}
                  </div>
                  <div className="detail" style={{ marginTop: 4 }}>
                    {scenarioBlurb(s, policy)}
                  </div>
                </button>
              ))}
            </div>

            {simError && (
              <div className="error-banner" role="alert">
                Simulation failed: {simError}
              </div>
            )}

            {result ? (
              <div className={`result ${result.view.allowed ? 'allow' : 'reject'}`}>
                <div className="status">
                  {result.view.allowed ? 'ALLOW' : 'REJECT'} · can_spend
                </div>
                <div className="reason">
                  reason: {result.view.reason}
                  {' · '}
                  {result.scenario.amountNear} Ⓝ → {result.scenario.receiver}
                </div>
                <div className="meta">
                  <span>spent_today: {formatNear(result.view.spent_today)}</span>
                  <span>remaining_daily: {formatNear(result.view.remaining_daily)}</span>
                  <span>day_bucket: {result.view.day_bucket}</span>
                  <span>at {result.at}</span>
                </div>
              </div>
            ) : (
              <div className="result">
                <p className="idle-hint" style={{ margin: 0 }}>
                  Pick a scenario to see the exact on-chain allow / reject.
                </p>
              </div>
            )}

            {log.length > 1 && (
              <div style={{ marginTop: 14 }}>
                <div
                  style={{
                    fontSize: '0.72rem',
                    fontWeight: 600,
                    letterSpacing: '0.06em',
                    textTransform: 'uppercase',
                    color: 'var(--ink-mute)',
                    marginBottom: 8,
                  }}
                >
                  Session dry-runs (real RPC only)
                </div>
                <ul style={{ margin: 0, paddingLeft: 18, fontSize: '0.82rem' }}>
                  {log.map((item, i) => (
                    <li key={`${item.at}-${item.scenario.id}-${i}`} style={{ marginBottom: 4 }}>
                      <span className="mono">
                        [{item.at}] {item.view.allowed ? 'ALLOW' : 'REJECT'} {item.view.reason} —{' '}
                        {item.scenario.amountNear}Ⓝ → {item.scenario.receiver}
                      </span>
                    </li>
                  ))}
                </ul>
              </div>
            )}
          </>
        ) : (
          <Architecture />
        )}
      </section>

      <section className="panel">
        <div className="panel-head">
          <h2>On-chain policy</h2>
          <a className="mono" href={EXPLORER_URL} target="_blank" rel="noreferrer">
            open explorer ↗
          </a>
        </div>
        {policy ? (
          <dl className="policy-grid">
            <div>
              <dt>Owner</dt>
              <dd>{policy.owner}</dd>
            </div>
            <div>
              <dt>Paused</dt>
              <dd>{policy.paused ? 'true' : 'false'}</dd>
            </div>
            <div>
              <dt>max_per_tx (yocto)</dt>
              <dd>{policy.max_per_tx}</dd>
            </div>
            <div>
              <dt>daily_limit (yocto)</dt>
              <dd>{policy.daily_limit}</dd>
            </div>
            <div style={{ gridColumn: '1 / -1' }}>
              <dt>Whitelist</dt>
              <dd>{policy.whitelist.length ? policy.whitelist.join(', ') : '(empty)'}</dd>
            </div>
          </dl>
        ) : (
          <p className="loading">{loading ? 'Loading policy from testnet…' : 'Policy unavailable.'}</p>
        )}
      </section>

      <footer className="footer">
        <span>Bound — Autonomy, within bounds.</span>
        <span className="mono">rpc.testnet.near.org · views only</span>
      </footer>
    </div>
  )
}

function Architecture() {
  return (
    <div>
      <p className="idle-hint" style={{ marginTop: 0 }}>
        Funds sit in the guardian. Agents hold a Function Call key scoped to{' '}
        <code className="mono">spend</code> only — native transfer is impossible for that key.
      </p>
      <div className="arch" aria-label="Guardian architecture">
        <div className="arch-node">
          <div className="kicker">01 · Agent</div>
          <h3>Scoped FC key</h3>
          <p>
            The demo key may call <code className="mono">spend</code> only. A Function Call key
            cannot Transfer NEAR at the protocol level; this contract does not check key type.
          </p>
        </div>
        <div className="arch-node accent">
          <div className="kicker">02 · Guardian</div>
          <h3>Bound contract</h3>
          <p>
            Public policy: max / tx, daily limit, whitelist. <code className="mono">can_spend</code>{' '}
            dry-runs before <code className="mono">spend</code>.
          </p>
        </div>
        <div className="arch-node">
          <div className="kicker">03 · Vendor</div>
          <h3>Whitelist only</h3>
          <p>Treasury leaves only to listed accounts, under limits — or via owner withdraw.</p>
        </div>
      </div>
      <svg
        viewBox="0 0 640 56"
        width="100%"
        height="56"
        style={{ marginTop: 14, display: 'block' }}
        aria-hidden
      >
        <defs>
          <marker id="arrow" markerWidth="8" markerHeight="8" refX="6" refY="3" orient="auto">
            <path d="M0,0 L6,3 L0,6 Z" fill="#e85d04" />
          </marker>
        </defs>
        <line x1="40" y1="28" x2="280" y2="28" stroke="#e85d04" strokeWidth="2" markerEnd="url(#arrow)" />
        <line x1="320" y1="28" x2="560" y2="28" stroke="#e85d04" strokeWidth="2" markerEnd="url(#arrow)" />
        <text x="120" y="18" fill="#6b6258" fontSize="11" fontFamily="IBM Plex Mono, monospace">
          spend(receiver, amount)
        </text>
        <text x="400" y="18" fill="#6b6258" fontSize="11" fontFamily="IBM Plex Mono, monospace">
          transfer if allowed
        </text>
        <rect x="0" y="18" width="36" height="20" rx="4" fill="#14110f" />
        <rect x="282" y="14" width="36" height="28" rx="6" fill="#e85d04" />
        <rect x="572" y="18" width="36" height="20" rx="4" fill="#14110f" />
      </svg>
    </div>
  )
}
