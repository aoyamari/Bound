import { describe, it, expect } from 'vitest'
import { nearToYocto, yoctoToNear, formatNear } from './near'

describe('yocto helpers', () => {
  it('converts NEAR to yocto', () => {
    expect(nearToYocto(0.1)).toBe('100000000000000000000000')
    expect(nearToYocto(0.25)).toBe('250000000000000000000000')
    expect(nearToYocto(0.3)).toBe('300000000000000000000000')
    expect(nearToYocto(1)).toBe('1000000000000000000000000')
  })

  it('converts yocto to NEAR display', () => {
    expect(yoctoToNear('250000000000000000000000')).toBe('0.25')
    expect(yoctoToNear('500000000000000000000000')).toBe('0.5')
    expect(formatNear('100000000000000000000000')).toBe('0.1 Ⓝ')
  })
})
