import { describe, it, expect } from 'vitest'
import { nearToYocto, yoctoToNear, formatNear } from './near'

describe('yocto helpers', () => {
  it('converts NEAR to yocto', () => {
    expect(nearToYocto(0.1)).toBe('100000000000000000000000')
    expect(nearToYocto(0.25)).toBe('250000000000000000000000')
    expect(nearToYocto(0.3)).toBe('300000000000000000000000')
    expect(nearToYocto(1)).toBe('1000000000000000000000000')
    expect(nearToYocto('0.1')).toBe('100000000000000000000000')
    expect(nearToYocto('0.000000000000000000000001')).toBe('1')
    expect(nearToYocto('1.000000000000000000000000')).toBe('1000000000000000000000000')
  })

  it('rejects values that would round or change sign', () => {
    expect(() => nearToYocto('-0.1')).toThrow(/decimal/)
    expect(() => nearToYocto('1.2.3')).toThrow(/decimal/)
    expect(() => nearToYocto('1e-7')).toThrow(/decimal/)
    expect(() => nearToYocto('0.1000000000000000000000001')).toThrow(/24/)
    expect(() => nearToYocto(0.0000001)).toThrow(/decimal string/)
    expect(nearToYocto('0.30000000000000004')).not.toBe(nearToYocto('0.3'))
  })

  it('converts yocto to NEAR display without rounding up', () => {
    expect(yoctoToNear('250000000000000000000000')).toBe('0.25')
    expect(yoctoToNear('500000000000000000000000')).toBe('0.5')
    expect(formatNear('100000000000000000000000')).toBe('0.1 Ⓝ')
    expect(yoctoToNear('1', 4)).toBe('0')
    expect(yoctoToNear('90000000000000000000', 4)).toBe('0')
    expect(yoctoToNear('1', 24)).toBe('0.000000000000000000000001')
  })
})
