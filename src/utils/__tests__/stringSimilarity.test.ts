import { describe, it, expect } from 'vitest'
import { levenshteinDistance, similarity, findBestMatch } from '../stringSimilarity'

describe('stringSimilarity', () => {
  describe('levenshteinDistance', () => {
    it('相同字符串距离为 0', () => {
      expect(levenshteinDistance('hello', 'hello')).toBe(0)
    })

    it('不同字符串计算正确距离', () => {
      expect(levenshteinDistance('kitten', 'sitting')).toBe(3)
    })

    it('空字符串处理', () => {
      expect(levenshteinDistance('', 'abc')).toBe(3)
      expect(levenshteinDistance('abc', '')).toBe(3)
    })

    it('单字符差异', () => {
      expect(levenshteinDistance('cat', 'bat')).toBe(1)
      expect(levenshteinDistance('cat', 'at')).toBe(1)
      expect(levenshteinDistance('cat', 'cart')).toBe(1)
    })
  })

  describe('similarity', () => {
    it('相同字符串相似度为 1', () => {
      expect(similarity('test', 'test')).toBe(1)
    })

    it('不同字符串相似度在 0-1 之间', () => {
      const sim = similarity('hello', 'hallo')
      expect(sim).toBeGreaterThan(0)
      expect(sim).toBeLessThan(1)
    })

    it('忽略大小写', () => {
      expect(similarity('Test', 'test')).toBe(1)
      expect(similarity('HELLO', 'hello')).toBe(1)
    })

    it('完全不同字符串相似度接近 0', () => {
      const sim = similarity('abc', 'xyz')
      expect(sim).toBeCloseTo(0, 1)
    })
  })

  describe('findBestMatch', () => {
    it('找到最佳匹配', () => {
      const target = { title: '稻香', artist: '周杰伦' }
      const candidates = [
        { title: '稻香', artist: '周杰伦' },
        { title: '青花瓷', artist: '周杰伦' },
        { title: '七里香', artist: '周杰伦' }
      ]
      
      const match = findBestMatch(target, candidates)
      expect(match).toBe(candidates[0])
    })

    it('相似度低于阈值返回 null', () => {
      const target = { title: '完全不同的歌', artist: '' }
      const candidates = [
        { title: '周杰伦', artist: '稻香' }
      ]
      
      const match = findBestMatch(target, candidates)
      expect(match).toBeNull()
    })

    it('空候选列表返回 null', () => {
      const target = { title: 'test', artist: 'artist' }
      const match = findBestMatch(target, [])
      expect(match).toBeNull()
    })

    it('忽略大小写匹配', () => {
      const target = { title: 'JAY', artist: 'CHOU' }
      const candidates = [
        { title: 'jay', artist: 'chou' }
      ]
      
      const match = findBestMatch(target, candidates)
      expect(match).toBe(candidates[0])
    })

    it('艺术家可选匹配', () => {
      const target = { title: '稻香', artist: '' }
      const candidates = [
        { title: '稻香', artist: '周杰伦' },
        { title: '稻香', artist: '' }
      ]
      
      const match = findBestMatch(target, candidates)
      expect(match).toBe(candidates[1])
    })
  })
})
