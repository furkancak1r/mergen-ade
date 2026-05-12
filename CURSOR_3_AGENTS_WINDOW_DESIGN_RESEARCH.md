# Cursor 3.0 Agents Window Görsel Tasarım Notları

Amaç: Mergen ADE’nin yerleşimini değiştirmeden, yalnızca görsel dili Cursor 3.0 Agents Window hissine yaklaştırmak.

## Genel Tasarım Dili

Cursor 3 Agents Window, klasik IDE görünümünden daha sade, ajan odaklı ve düşük kontrastlı bir arayüz kullanıyor.

Öne çıkan karakter:
- Minimal chrome
- Geniş ama sade yüzeyler
- İnce border’lar
- Ghost / icon-only butonlar
- Hafif yuvarlatılmış köşeler
- Az doygun, nötr renk paleti
- Aktif alanlarda çok hafif dolgu farkı
- Hover durumlarında belirgin ama agresif olmayan highlight

## Layout Karakteri

Mergen’de yerleşim değişmeyecek; sadece aşağıdaki görsel his taklit edilmeli:

- Sol tarafta ajan/proje/oturum odaklı sidebar hissi
- Ana alanda tiled/grid agent panel mantığı
- Paneller arasında keskin ayırıcı yerine ince, düşük kontrast çizgiler
- Araç butonları mümkün olduğunca icon-first
- Aktif sekme/panel çok güçlü renkle değil, subtle background + text contrast ile vurgulanmalı
- Scrollbar ince ve düşük kontrastlı kalmalı

## Tema Sistemi Bulgusu

Cursor 3 Agents Window kendi tema motorunu kullanıyor.
VS Code temaları Agents Window’da doğrudan desteklenmiyor.
Forum açıklamasına göre şu an sadece:
- Basic Light
- Basic Dark
- Font ayarları

destekleniyor.

Bu yüzden Mergen tarafında “Cursor birebir theme import” değil, token tabanlı görsel benzetme yapılmalı.

## Renk Paleti

### Light Theme Kaynaklı Gözlem

Resmi Cursor ekran görüntülerinde light tema baskın.

Yaklaşık renkler:

| Rol | Renk |
| --- | --- |
| App background | `#FCFCFC` |
| Surface | `#F8F8F8` |
| Surface soft | `#F0F0F0` |
| Border | `#E0E0E0` / `#D8D8D8` |
| Muted border | `#C8C8C8` |
| Primary text | `#1F2328` |
| Muted text | `#6A6A6A` |
| Accent blue | `#3078F8` |

### Dark Theme / Forum Görsellerinden Gözlem

Dark tema çok siyah değil; çoğunlukla koyu nötr gri / lacivert-gri yüzeyler var.

Yaklaşık renkler:

| Rol | Renk |
| --- | --- |
| App background | `#101010` / `#141414` |
| Main surface | `#181818` |
| Secondary surface | `#1B1F2A` |
| Soft surface | `#202020` / `#202830` |
| Elevated surface | `#303030` |
| Border | `#2A2A2A` / `#30343A` |
| Hover fill | `#2B2B2B` / `#303030` |
| Active fill | `#383838` |
| Primary text | `#F4F4F4` |
| Secondary text | `#C8C8C8` |
| Muted text | `#8A8A8A` |
| Disabled text | `#606060` |
| Accent blue | `#3078F8` |
| Secondary blue | `#1070A8` |

## Mergen İçin Önerilen Dark Token Set

Mergen şu an pure dark tarafa yakın. Cursor hissi için siyah/beyaz kontrast biraz yumuşatılmalı.

```rust
APP_BG             = #101010
SURFACE_BG         = #181818
SURFACE_BG_SOFT    = #202020
TERMINAL_OUTPUT_BG = #141414
BORDER_COLOR       = #2A2A2A
ACCENT             = #C8C8C8
TEXT_PRIMARY       = #F4F4F4
TEXT_MUTED         = #8A8A8A
HOVER_BG           = #2B2B2B
ACTIVE_BG          = #383838
BLUE_ACCENT        = #3078F8
```

## Buton Görsel Kuralları

Mevcut buton yerleri değişmemeli.

Cursor benzeri görünüm için:

- Inactive icon button:
  - Background transparent
  - Icon opacity yaklaşık %65-75
- Hover:
  - Rounded rect dolgu
  - `#2B2B2B` veya alpha’lı `#303030`
  - Border yok veya çok düşük kontrast
- Active/selected:
  - Hafif dolgu: `#303030` / `#383838`
  - Text/icon `#FFFFFF` yerine `#F4F4F4`
- Primary action:
  - Doygun mavi yerine kontrollü accent
  - `#3078F8` sadece gerçekten önemli actionlarda
- Destructive:
  - Kırmızı çok parlak olmamalı
  - Background yerine çoğunlukla text/icon kırmızı yeterli

## Köşe Yuvarlama

Cursor Agents Window’da köşeler modern ama aşırı yuvarlak değil.

Öneri:

| Eleman | Radius |
| --- | --- |
| App/panel window | 10-12px |
| Cards / surfaces | 8-10px |
| Icon buttons | 6-8px |
| Inputs | 6-8px |
| Small badges | 999px / pill |

## Spacing

Mevcut layout korunmalı.

Sadece görsel sıkılık Cursor’a yaklaştırılabilir:

- Button padding: 8x5 veya 10x6
- Panel iç margin: 6-10px
- Toolbar gap: 4-6px
- Sidebar row height: kompakt, 28-34px
- Border kalınlığı: 1px
- Scrollbar: ince, düşük kontrast

## Typography

- Sistem fontu hissi korunmalı.
- Başlıklar çok büyük olmamalı.
- UI text 12-14px aralığında kalmalı.
- Secondary metinlerde muted gri kullanılmalı.
- Bold sadece panel başlığı / önemli state için kullanılmalı.

## Mergen’de Dokunulacak Ana Noktalar

Öncelikli dosya:
- `src/app.rs`

Önemli bölgeler:
- Tema sabitleri: `APP_BG`, `SURFACE_BG`, `SURFACE_BG_SOFT`, `BORDER_COLOR`, `ACCENT`, `TEXT_PRIMARY`, `TEXT_MUTED`
- `ensure_theme_initialized()`
- `styled_icon_button`
- `activity_rail_icon_button`
- `browser_toolbar_icon_button`
- `styled_icon_button_response`
- `settings_surface_frame`
- Smart Input footer frame
- Toast/modal frame stilleri

## Uygulama İlkesi

Yerleşim değişmeyecek.

Yapılacaklar:
- Renk tokenlarını Cursor-like hale getir
- Hover/active state’leri yumuşat
- Button chrome’u azalt
- Border kontrastını düşür
- Text beyazını kırık beyaza çek
- Accent renkleri daha kontrollü kullan
- Modal/card yüzeylerini daha nötr hale getir

Yapılmayacaklar:
- Butonların yerini değiştirme
- Panel sıralamasını değiştirme
- Toolbar akışını değiştirme
- Terminal davranışını değiştirme
- Browser / terminal / worktree logic’e dokunma

## Kabul Kriterleri

- Mergen’in layout’u aynı kalır.
- Sadece görünüm değişir.
- Dark tema Cursor Agents Window hissine yaklaşır.
- Hover/active/focus durumları hâlâ anlaşılırdır.
- Text kontrastı okunabilir kalır.
- `cargo fmt` ve `cargo test` geçer.
- Manuel kontrolde:
  - Activity rail
  - Terminal Manager
  - Browser toolbar
  - Settings popup
  - Smart Input footer
  - File editor header
  - Terminal panes

  görsel olarak uyumlu görünür.
