import os
from PIL import Image, ImageDraw
import math

def create_icon(size):
    width = height = size
    
    img = Image.new('RGBA', (width, height), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    
    # 背景 - 绿色
    bg_color = (29, 185, 84)
    corner_radius = int(size * 0.2)
    mask = Image.new('L', (width, height), 0)
    mask_draw = ImageDraw.Draw(mask)
    mask_draw.rounded_rectangle([(0, 0), (width, height)], corner_radius, fill=255)
    
    # 填充背景
    draw.rounded_rectangle([(0, 0), (width, height)], corner_radius, fill=bg_color)
    img.putalpha(mask)
    
    scale = size / 512
    
    # 耳机
    headphone_center_x = int(230 * scale)  # 调整位置
    headphone_center_y = int(200 * scale)
    headphone_scale = 0.9 * scale
    
    # 耳机外圈
    headphone_radius = int(140 * headphone_scale)
    headphone_stroke_width = int(20 * headphone_scale)
    
    # 绘制耳机外圈
    draw.ellipse([
        headphone_center_x - headphone_radius,
        headphone_center_y - headphone_radius,
        headphone_center_x + headphone_radius,
        headphone_center_y + headphone_radius
    ], outline=(255, 255, 255), width=headphone_stroke_width)
    
    # 耳机内部
    inner_circle_radius = int(80 * headphone_scale)
    draw.ellipse([
        headphone_center_x - inner_circle_radius,
        headphone_center_y - inner_circle_radius,
        headphone_center_x + inner_circle_radius,
        headphone_center_y + inner_circle_radius
    ], fill=(255, 255, 255))
    
    # 耳机中心
    center_circle_radius = int(30 * headphone_scale)
    draw.ellipse([
        headphone_center_x - center_circle_radius,
        headphone_center_y - center_circle_radius,
        headphone_center_x + center_circle_radius,
        headphone_center_y + center_circle_radius
    ], fill=bg_color)
    
    # 音符
    note_center_x = int(260 * scale)
    note_center_y = int(280 * scale)
    note_scale = 1.1 * scale
    
    # 音符外圆
    note_radius = int(90 * note_scale)
    draw.ellipse([
        note_center_x - note_radius,
        note_center_y - note_radius,
        note_center_x + note_radius,
        note_center_y + note_radius
    ], fill=(255, 255, 255))
    
    # 音符内部
    note_inner_width = int(40 * note_scale)
    note_inner_height = int(60 * note_scale)
    note_inner_x = note_center_x - note_inner_width // 2
    note_inner_y = note_center_y - note_inner_height // 2
    
    draw.rectangle([
        note_inner_x,
        note_inner_y,
        note_inner_x + note_inner_width,
        note_inner_y + note_inner_height
    ], fill=bg_color)
    
    return img

sizes = [32, 128, 256, 512, 1024]
output_dir = r'e:\TPlayer\src-tauri\icons'

for size in sizes:
    icon = create_icon(size)
    
    if size == 32:
        filename = '32x32.png'
    elif size == 128:
        filename = '128x128.png'
    elif size == 256:
        filename = 'icon.png'
    elif size == 512:
        filename = 'icon_512.png'
    elif size == 1024:
        filename = 'icon_1024.png'
    
    filepath = os.path.join(output_dir, filename)
    icon.save(filepath, 'PNG')
    print(f'Generated {filename}')

# 生成128x128@2x.png（256x256）
icon_256 = create_icon(256)
icon_512 = icon_256.resize((512, 512), Image.Resampling.LANCZOS)
icon_512.save(os.path.join(output_dir, '128x128@2x.png'), 'PNG')
print('Generated 128x128@2x.png')

# 生成场景图标（Square*Logo.png）
square_sizes = [30, 44, 71, 89, 107, 142, 150, 284, 310]
for size in square_sizes:
    icon = create_icon(size)
    filename = f'Square{size}x{size}Logo.png'
    filepath = os.path.join(output_dir, filename)
    icon.save(filepath, 'PNG')
    print(f'Generated {filename}')

# 生成StoreLogo.png
store_logo = create_icon(150)
store_logo.save(os.path.join(output_dir, 'StoreLogo.png'), 'PNG')
print('Generated StoreLogo.png')

print('All icons generated successfully!')
