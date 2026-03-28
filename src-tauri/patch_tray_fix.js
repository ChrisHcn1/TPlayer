const fs = require('fs');
const path = 'e:\\TPlayer\\src\\App.vue';

let content = fs.readFileSync(path, 'utf8');

// 移除前端的系统托盘初始化代码

// 1. 移除 import TrayIcon
content = content.replace(/import { TrayIcon } from '@tauri-apps\/api\/tray'/g, '');

// 2. 移除 trayIcon 变量
content = content.replace(/let trayIcon: TrayIcon \| null = null/g, '');

// 3. 移除 initSystemTray 函数
const initTrayFunction = /const initSystemTray = async \(\) => \{[\s\S]*?\}/g;
content = content.replace(initTrayFunction, '');

// 4. 移除调用 initSystemTray 的代码
content = content.replace(/await initSystemTray\(\)/g, '// 系统托盘已在后端初始化');

fs.writeFileSync(path, content, 'utf8');
console.log('前端系统托盘初始化代码已移除');
