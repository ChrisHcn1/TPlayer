#!/usr/bin/env node
/**
 * sync-release.mjs —— 把 GitHub Release 的版本信息导出为网站可消费的 JSON 快照。
 *
 * 用法：
 *   node scripts/sync-release.mjs                          # 默认仓库 + 默认输出路径
 *   node scripts/sync-release.mjs --repo owner/name --out site/releases.json --limit 10
 *
 * 环境变量：
 *   GITHUB_TOKEN / GH_TOKEN   可选。用于提高 API 限额（公开仓库可省略）。
 *
 * 设计要点：
 *  - 零依赖（只用 Node 18+ 内置 fetch / fs），CI 与本地都能跑；
 *  - **幂等**：内容（除 updated_at 外）与既有快照一致时不写盘，
 *    避免 CI 里产生无意义的空提交；
 *  - 只取已发布的 Release（draft 跳过），pre-release 保留并打标记；
 *  - 同时保留 assets（安装包）列表，网站可直接给出下载入口。
 */
import { readFile, writeFile, mkdir } from 'node:fs/promises'
import { dirname, resolve } from 'node:path'

function parseArgs(argv) {
  const args = {
    repo: process.env.TPLAYER_REPO || 'ChrisHcn1/TPlayer',
    out: 'site/releases.json',
    token: process.env.GITHUB_TOKEN || process.env.GH_TOKEN || '',
    limit: 10,
  }
  for (let i = 2; i < argv.length; i++) {
    const a = argv[i]
    if (a === '--repo') args.repo = argv[++i]
    else if (a === '--out') args.out = argv[++i]
    else if (a === '--token') args.token = argv[++i]
    else if (a === '--limit') args.limit = Number(argv[++i])
    else if (a === '--help' || a === '-h') {
      console.log('usage: node scripts/sync-release.mjs [--repo owner/name] [--out file] [--limit N]')
      process.exit(0)
    }
  }
  return args
}

/** 从 tag 里取出裸版本号（v2.0.3 -> 2.0.3） */
function bareVersion(tag) {
  return String(tag || '').replace(/^v/i, '')
}

/** 归一化一条 Release 为快照条目 */
function toEntry(r) {
  const assets = (r.assets || []).map((a) => ({
    name: a.name,
    size: a.size,
    url: a.browser_download_url,
  }))
  return {
    version: bareVersion(r.tag_name),
    tag: r.tag_name,
    name: r.name || r.tag_name,
    published_at: r.published_at || r.created_at || null,
    url: r.html_url,
    prerelease: !!r.prerelease,
    body: r.body || '',
    assets,
  }
}

/** 挑选「主安装包」资产：优先 nsis exe / msix / msi / dmg / deb / AppImage */
function pickInstaller(entry) {
  const order = ['setup.exe', '.msix', '.msi', '.dmg', '.deb', '.appimage', '.exe']
  for (const ext of order) {
    const hit = entry.assets.find((a) => a.name.toLowerCase().endsWith(ext))
    if (hit) return hit
  }
  return entry.assets[0] || null
}

async function fetchReleases(repo, token) {
  const headers = {
    accept: 'application/vnd.github+json',
    'x-github-api-version': '2022-11-28',
    'user-agent': 'tplayer-sync-release',
  }
  if (token) headers.authorization = 'Bearer ' + token
  const url = 'https://api.github.com/repos/' + repo + '/releases?per_page=20'
  const res = await fetch(url, { headers })
  if (!res.ok) {
    throw new Error('GitHub API ' + res.status + ' ' + res.statusText + ' — ' + url)
  }
  const list = await res.json()
  if (!Array.isArray(list)) throw new Error('GitHub API 返回格式异常')
  return list
}

async function main() {
  const args = parseArgs(process.argv)
  const releases = (await fetchReleases(args.repo, args.token))
    .filter((r) => !r.draft)
    .map(toEntry)
    .slice(0, Math.max(1, args.limit))

  if (!releases.length) {
    console.warn('[sync-release] 没有已发布的 Release，跳过（保留既有快照）')
    return
  }

  const payload = {
    updated_at: new Date().toISOString(),
    repo: args.repo,
    latest: Object.assign({}, releases[0], { installer: pickInstaller(releases[0]) }),
    releases: releases.map((e) => Object.assign({}, e, { installer: pickInstaller(e) })),
  }

  const outPath = resolve(process.cwd(), args.out)
  const body = JSON.stringify(payload, null, 2) + '\n'

  // 幂等：忽略 updated_at 后内容一致则不写盘
  let unchanged = false
  try {
    const prev = JSON.parse(await readFile(outPath, 'utf8'))
    const strip = (o) => JSON.stringify(Object.assign({}, o, { updated_at: '' }))
    unchanged = strip(prev) === strip(payload)
  } catch {
    unchanged = false
  }
  if (unchanged) {
    console.log('[sync-release] 快照无变化，保持原文件：' + args.out)
    return
  }

  await mkdir(dirname(outPath), { recursive: true })
  await writeFile(outPath, body, 'utf8')
  console.log('[sync-release] 已写入 ' + args.out)
  console.log('[sync-release] 最新版本 v' + payload.latest.version + '（' + payload.latest.published_at + '），共 ' + payload.releases.length + ' 条记录')
}

main().catch((err) => {
  console.error('[sync-release] 失败：', err.message)
  process.exit(1)
})
