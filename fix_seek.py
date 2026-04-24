import re

# 读取文件
with open(r'e:\TPlayer\src\App.vue', 'r', encoding='utf-8') as f:
    content = f.read()

# 找到 seek 函数中的 invoke 调用并注释掉
# 定位到 seek 函数中的定时器启动部分
old_seek_timer = r'''          // 确保 FFplay 状态监控定时器正在运行
          if (!ffplayStatusInterval) {
            console.log('【SEEK】FFplay 状态监控定时器未运行，启动一个新的')
            logInfo('FFplay 状态监控定时器未运行，启动一个新的')
            // 启动 FFplay 状态监控定时器
            ffplayStatusInterval = window.setInterval\(\(\) => \{
              console\.log\('【FFplay】状态监控定时器触发'\)
              logInfo\('FFplay 状态监控定时器触发'\)
              // 使用 IIFE 包装 async 函数
              \(async \(\) => \{
                try \{
                  console\.log\('【FFplay】准备调用 get_ffplay_status'\)
                  logInfo\('准备调用 get_ffplay_status'\)
                  const status = await invoke\('get_ffplay_status'\) as any
                  console\.log\('【FFplay】获取 FFplay 状态成功:', JSON\.stringify\(status\)\)
                  logInfo\('获取 FFplay 状态成功:', JSON\.stringify\(status\)\)

                  if \(status\) \{
                    console\.log\('【FFplay】处理 FFplay 状态:', \{
'''

new_seek_timer = '''          // 不再启动定时器，seek 后由前端定时器自动同步进度
'''

# 替换
new_content = re.sub(re.escape('          // 确保 FFplay 状态监控定时器正在运行\n          if (!ffplayStatusInterval) {\n            console.log(\'【SEEK】FFplay 状态监控定时器未运行，启动一个新的\')\n            logInfo(\'FFplay 状态监控定时器未运行，启动一个新的\')\n            // 启动 FFplay 状态监控定时器\n            ffplayStatusInterval = window.setInterval(() => {\n              console.log(\'【FFplay】状态监控定时器触发\')\n              logInfo(\'FFplay 状态监控定时器触发\')\n              // 使用 IIFE 包装 async 函数\n              (async () => {\n                try {\n                  console.log(\'【FFplay】准备调用 get_ffplay_status\')\n                  logInfo(\'准备调用 get_ffplay_status\')\n                  const status = await invoke(\'get_ffplay_status\') as any\n                  console.log(\'【FFplay】获取 FFplay 状态成功:\', JSON.stringify(status))\n                  logInfo(\'获取 FFplay 状态成功:\', JSON.stringify(status))\n\n                  if (status) {\n                    console.log(\'【FFplay】处理 FFplay 状态:\', {'), new_seek_timer, content)

# 写回文件
with open(r'e:\TPlayer\src\App.vue', 'w', encoding='utf-8') as f:
    f.write(new_content)

print('✅ 文件修改成功 - 已移除 seek 后的定时器启动')
