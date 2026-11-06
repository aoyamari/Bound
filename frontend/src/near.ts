/** Public NEAR testnet view helpers — no keys, no wallet. */

export const CONTRACT_ID = 'guardian.bound-aoyamari.testnet'
export const RPC_URL = 'https://rpc.testnet.near.org'
export const EXPLORER_URL = `https://explorer.testnet.near.org/accounts/${CONTRACT_ID}`
export const YOCTO_PER_NEAR = 10n ** 24n

export type PolicyView = {
  owner: string
  max_per_tx: string
  daily_limit: string
  paused: boolean
  whitelist: string[]
}

export type CanSpendView = {
  allowed: boolean
  reason: string
  spent_today: string
  remaining_daily: string
  day_bucket: number
}

function encodeArgs(args: Record<string, unknown>): string {
  const json = JSON.stringify(args)
  const bytes = new TextEncoder().encode(json)
  let binary = ''
  for (const b of bytes) binary += String.fromCharCode(b)
  return btoa(binary)
}

function decodeResult(result: number[]): unknown {
  const bytes = new Uint8Array(result)
  const text = new TextDecoder().decode(bytes)
  return JSON.parse(text)
}

export async function viewFunction<T>(
  methodName: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  const body = {
    jsonrpc: '2.0',
    id: 'bound-dashboard',
    method: 'query',
    params: {
      request_type: 'call_function',
      finality: 'optimistic',
      account_id: CONTRACT_ID,
      method_name: methodName,
      args_base64: encodeArgs(args),
    },
  }

  const res = await fetch(RPC_URL, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  })

  if (!res.ok) {
    throw new Error(`RPC HTTP ${res.status}`)
  }

  const json = (await res.json()) as {
    result?: { result: number[]; logs?: string[] }
    error?: { message?: string; data?: string }
  }

  if (json.error) {
    throw new Error(json.error.data || json.error.message || 'RPC error')
  }
  if (!json.result?.result) {
    throw new Error('Empty RPC result')
  }

  return decodeResult(json.result.result) as T
}

export function getPolicy(): Promise<PolicyView> {
  return viewFunction<PolicyView>('get_policy')
}

export function getSpentToday(): Promise<string> {
  return viewFunction<string>('get_spent_today')
}

export function canSpend(receiver: string, amountYocto: string): Promise<CanSpendView> {
  return viewFunction<CanSpendView>('can_spend', {
    receiver,
    amount: amountYocto,
  })
}

function decimalString(near: number | string): string {
  if (typeof near === 'number') {
    if (!Number.isFinite(near) || near < 0) {
      throw new Error('NEAR amount must be a finite non-negative number')
    }
    const rendered = near.toString()
    if (/[eE]/.test(rendered)) {
      throw new Error('pass this NEAR amount as a decimal string, for example "0.1"')
    }
    return rendered
  }
  return near.trim()
}

/** Exact yoctoNEAR. Fractional digits past 24 are rejected rather than rounded. */
export function nearToYocto(near: number | string): string {
  const s = decimalString(near)
  const match = /^(0|[1-9]\d*)(?:\.(\d+))?$/.exec(s)
  if (!match) {
    throw new Error('NEAR amount must be a base-10 decimal string, for example "0.1"')
  }
  const whole = match[1]
  const frac = match[2] ?? ''
  if (frac.length > 24) {
    throw new Error('NEAR amount has more than 24 decimal places')
  }
  const fracPadded = frac.padEnd(24, '0')
  return (BigInt(whole) * YOCTO_PER_NEAR + BigInt(fracPadded)).toString()
}

export function yoctoToNear(yocto: string | number | bigint, digits = 4): string {
  if (typeof yocto === 'number' && !Number.isSafeInteger(yocto)) {
    throw new Error('pass yoctoNEAR as a string or bigint')
  }
  const v = BigInt(yocto)
  const whole = v / YOCTO_PER_NEAR
  const frac = v % YOCTO_PER_NEAR
  if (digits <= 0) return whole.toString()
  const fracStr = frac.toString().padStart(24, '0').slice(0, digits).replace(/0+$/, '')
  return fracStr ? `${whole}.${fracStr}` : whole.toString()
}

export function formatNear(yocto: string | number | bigint): string {
  return `${yoctoToNear(yocto, 4)} Ⓝ`
}
