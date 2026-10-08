import { describe, it, expect, vi } from 'vitest'
// mock parseSmartLrc 以分别测试主路径与回退分支
vi.mock('../../services/lyricParser', () => ({
  parseSmartLrc: vi.fn()
}))

import { parseSmartLrc } from '../../services/lyricParser'
import { parseLyrics, toSimpleLyricLines } from '../lyrics'

describe('parseLyrics', () => {
  it('空串返回空数组且不调用 parseSmartLrc', () => {
    vi.mocked(parseSmartLrc).mockClear()
    expect(parseLyrics('')).toEqual([])
    expect(parseSmartLrc).not.toHaveBeenCalled()
  })

  it('主路径：parseSmartLrc 成功时用其结构化结果（毫秒转秒）', () => {
    vi.mocked(parseSmartLrc).mockReturnValue({
      format: 'line',
      lines: [
        { startTime: 1500, words: [{ word: '你好' }] },
        { startTime: 3000, words: [{ word: '世界' }] }
      ]
    } as any)
    expect(parseLyrics('[00:01.50]你好')).toEqual([
      { time: 1.5, text: '你好' },
      { time: 3, text: '世界' }
    ])
  })

  it('回退分支：parseSmartLrc 抛错时走正则解析', () => {
    vi.mocked(parseSmartLrc).mockImplementation(() => { throw new Error('fail') })
    const result = parseLyrics('[00:01.50]第一行\n[00:03.00]第二行')
    expect(result).toEqual([
      { time: 1.5, text: '第一行' },
      { time: 3, text: '第二行' }
    ])
  })

  it('多时间戳单行拆成多行（共享文本）', () => {
    vi.mocked(parseSmartLrc).mockImplementation(() => { throw new Error('multi') })
    expect(parseLyrics('[00:01.00][00:02.00]共享歌词')).toEqual([
      { time: 1, text: '共享歌词' },
      { time: 2, text: '共享歌词' }
    ])
  })

  it('回退分支按时间升序排序（乱序输入）', () => {
    vi.mocked(parseSmartLrc).mockImplementation(() => { throw new Error('sort') })
    const result = parseLyrics('[00:03.00]c\n[00:01.00]a\n[00:02.00]b')
    expect(result.map(r => r.text)).toEqual(['a', 'b', 'c'])
  })

  it('无时间戳的纯文本行被忽略', () => {
    vi.mocked(parseSmartLrc).mockImplementation(() => { throw new Error('no-ts') })
    expect(parseLyrics('纯文本行无时间戳')).toEqual([])
  })

  it('空文本行被忽略（有时间戳但 text 为空）', () => {
    vi.mocked(parseSmartLrc).mockImplementation(() => { throw new Error('empty-text') })
    expect(parseLyrics('[00:01.00]   ')).toEqual([])
  })
})

describe('toSimpleLyricLines', () => {
  it('毫秒转秒并拼接 word', () => {
    const input = [{ startTime: 1500, words: [{ word: 'a' }, { word: 'b' }] }]
    expect(toSimpleLyricLines(input as any)).toEqual([{ time: 1.5, text: 'ab' }])
  })

  it('空数组返回空数组', () => {
    expect(toSimpleLyricLines([])).toEqual([])
  })
})
