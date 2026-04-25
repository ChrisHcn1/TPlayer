/**
 * 计算两个字符串的 Levenshtein 距离
 */
export function levenshteinDistance(str1: string, str2: string): number {
  const m = str1.length
  const n = str2.length
  const dp: number[][] = Array(m + 1).fill(null).map(() => Array(n + 1).fill(0))

  for (let i = 0; i <= m; i++) dp[i][0] = i
  for (let j = 0; j <= n; j++) dp[0][j] = j

  for (let i = 1; i <= m; i++) {
    for (let j = 1; j <= n; j++) {
      if (str1[i - 1] === str2[j - 1]) {
        dp[i][j] = dp[i - 1][j - 1]
      } else {
        dp[i][j] = Math.min(
          dp[i - 1][j] + 1,     // 删除
          dp[i][j - 1] + 1,     // 插入
          dp[i - 1][j - 1] + 1  // 替换
        )
      }
    }
  }

  return dp[m][n]
}

/**
 * 计算相似度（0-1 之间）
 */
export function similarity(str1: string, str2: string): number {
  const maxLen = Math.max(str1.length, str2.length)
  if (maxLen === 0) return 1
  
  const distance = levenshteinDistance(str1.toLowerCase(), str2.toLowerCase())
  return 1 - distance / maxLen
}

/**
 * 查找最佳匹配
 */
export function findBestMatch<T extends { title: string; artist?: string }>(
  target: { title: string; artist?: string },
  candidates: T[]
): T | null {
  if (candidates.length === 0) return null

  const targetKey = `${target.title} ${target.artist || ''}`.toLowerCase()
  
  let bestMatch: T | null = null
  let bestScore = 0

  for (const candidate of candidates) {
    const candidateKey = `${candidate.title} ${candidate.artist || ''}`.toLowerCase()
    const score = similarity(targetKey, candidateKey)
    
    if (score > bestScore && score >= 0.6) {
      bestScore = score
      bestMatch = candidate
    }
  }

  return bestMatch
}
