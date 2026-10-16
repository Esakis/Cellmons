"""Original final-packet SVG compositions using the project's own geometric vocabulary.
School emblems and object fragments are shared deliberately, as in the other sets.
No external images, fonts, or traced material enter these illustrations.
"""
from pathlib import Path
from html import escape
import importlib.util
import tomllib
ROOT=Path(__file__).resolve().parent.parent

def module(name):
 spec=importlib.util.spec_from_file_location(name,Path(__file__).with_name('generate-'+name+'-art.py'))
 result=importlib.util.module_from_spec(spec);spec.loader.exec_module(result);return result
relic=module('relic');eclipse=module('eclipse');covenant=module('covenant')
PALETTE,GLYPHS=covenant.PALETTE,covenant.GLYPHS
R=relic.DRAWINGS;E=eclipse.DRAWINGS;C=covenant.DRAWINGS
# Object selection is explicit and reviewable rather than random or keyed by a hash.
RELIC_OBJECTS=[
 R['sword_oath'],R['living_elixir'],R['wild_current'],E['pale_index'],E['cold_obol'],C['convergence_engine'],
 C['final_brand'],C['dawn_covenant'],R['wild_current'],E['margin_note'],R['grave_halo'],R['calming_regulator'],
 E['margin_note'],E['grave_ration'],E['silt_veil'],E['pale_index'],R['night_rite'],C['aether_reactor'],
 R['martial_staff'],R['mending_halo'],R['ember_rain'],E['margin_note'],R['night_rite'],E['winding_key'],
 E['washer_ward'],E['quiet_toll'],E['funeral_knot'],R['sage_stone'],E['quiet_toll'],E['winding_key'],
 C['red_horizon'],R['mending_halo'],C['aether_reactor']]
UTILITY_OBJECTS=[E['split_guard'],E['votive_thread'],E['dew_pocket'],E['margin_note'],E['sable_splinter'],E['spare_capacitor']]

def emblem(school,x,y,scale=.45):
 dark,light=PALETTE[school]
 return f'<g transform="translate({x} {y}) scale({scale})" fill="{light}" stroke="{light}" style="--shade-detail:{dark}"><circle r="26" fill="{dark}" stroke="none"/>{GLYPHS[school]}</g>'
def objects(svg,scale=.66,x=27,y=17):return f'<g transform="translate({x} {y}) scale({scale})">{svg}</g>'
def cards_fan(n):
 return ''.join(f'<path d="M{37+i*16} {19+i*3}h37v59h-37Z"/><path d="M{44+i*16} {32+i*3}h23m-23 10h23m-23 10h17" fill="none" stroke="var(--dark)"/>'for i in range(n))
def rays(n,r=43):
 import math
 return ''.join(f'<path d="M{80+math.cos(i*2*math.pi/n)*(r-5):.2f} {50+math.sin(i*2*math.pi/n)*(r-5):.2f}L{80+math.cos(i*2*math.pi/n)*(r+4):.2f} {50+math.sin(i*2*math.pi/n)*(r+4):.2f}"/>' for i in range(n))
def drawing(card,i):
 schools=list(card['requires']);school=schools[0]
 if card['set']=='relic':
  main=objects(RELIC_OBJECTS[i],.63,30,18)
  if i<6:
   rim='<ellipse cx="36" cy="50" rx="17" ry="27" fill="none"/><ellipse cx="123" cy="50" rx="17" ry="27" fill="none"/><path d="M17 50H33M126 50H143" fill="none"/>'
  elif i<12:
   rim='<path d="M23 75Q7 42 30 22M137 75Q153 42 130 22M22 34L34 35 28 23M19 51L32 48M24 67L37 59M138 34L126 35 132 23M141 51L128 48M136 67L123 59M42 89L80 80 119 89" fill="none"/>'
  elif i<18:
   rim='<path d="M15 72L20 22 44 25M116 25L141 22 146 73M20 83L49 86M111 86L141 83" fill="none"/>'
  elif i<24:
   rim='<path d="M21 22L37 38M139 22L123 38M22 80L39 66M139 80L122 67M15 44L27 50 16 57M145 44L133 50 144 57" fill="none"/>'
  elif i<30:
   rim='<path d="M14 53H25L32 38 39 63 46 53M114 53H122L130 38 137 63 146 53" fill="none"/>'
  else:rim='<circle cx="80" cy="50" r="42" fill="none"/>'+rays(9)
  return rim+main+f'<circle cx="132" cy="82" r="12" fill="var(--dark)"/>'+emblem(school,132,82,.35)
 if card['set']=='covenant':
  if i<15:
   a,b=schools
   bridges=[f'<path d="M{24+j*11} {78+(j%2)*8}L{34+j*11} {86-(j%2)*8}"/>' for j in range(10)]
   arch=f'<path d="M25 {26+i%3*4}Q80 {3+i%5*3} 135 {26+i%3*4}M25 {72-i%3*4}Q80 {97-i%5*3} 135 {72-i%3*4}" fill="none"/>'
   return arch+'<circle cx="49" cy="47" r="24" fill="var(--dark)"/><circle cx="112" cy="47" r="24" fill="var(--dark)"/>'+emblem(a,49,47,.66)+emblem(b,112,47,.66)+'<path d="M73 47H87" fill="none" stroke-width="5"/>'+''.join(bridges)
  if i<21:
   return '<path d="M80 7L111 28V69L80 92 49 69V28Z"/><path d="M80 16L101 32V65L80 82 59 65V32Z" fill="var(--dark)"/>'+emblem(school,80,49,.76)+rays(4,43)
  # Unequal scales depict a permanent gain bought with another resource.
  t=i-21
  objects_pair=[(R['sword_oath'],E['split_guard']),(E['split_guard'],R['astral_scepter']),(R['primal_heart'],R['sword_oath']),(E['washer_ward'],E['spare_capacitor']),(R['astral_scepter'],E['split_guard']),(E['split_guard'],E['spare_capacitor']),(E['pale_index'],R['primal_heart']),(R['astral_scepter'],E['washer_ward']),(E['cold_obol'],E['washer_ward']),(E['split_guard'],R['primal_heart']),(R['astral_scepter'],R['primal_heart']),(E['split_guard'],E['spare_capacitor'])][t]
  detail=objects(objects_pair[0],.22,25,45)+objects(objects_pair[1],.22,101,57)
  return f'<path d="M80 15V87M58 91H103M28 {31+t%3*3}L132 {47-t%3*3}M43 35L25 65H62ZM119 45L101 77H138Z"/><circle cx="80" cy="16" r="8"/><path d="M34 66h20m55 12h20" fill="none" stroke="var(--dark)"/>'+emblem(school,80,51,.47)+'<g fill="var(--dark)" stroke-width="3">'+detail+'</g>'
 if i<6:
  return '<path d="M61 13H99V30L114 49V82Q80 96 46 82V49L61 30Z"/><path d="M54 76H106M63 21H96M78 32L70 50 88 61 76 79" fill="none" stroke="var(--dark)"/>'+emblem(school,80,54,.44)+'<path d="M24 27L36 38M136 27L124 38M24 77L35 67M137 77L126 67" fill="none"/>'
 if i<12:
  return cards_fan(3)+emblem(school,91,52,.43)+f'<path d="M21 {84-i%3*3}Q80 98 138 84" fill="none"/>'
 if i<18:
  return '<path d="M45 22L80 9 117 22V58Q104 79 80 92 57 80 45 58Z"/><path d="M81 17V76L106 55V29Z" fill="var(--dark)"/><path d="M57 28V53M57 60L68 70" fill="none"/>'+emblem(school,80,47,.5)
 if i<24:
  return '<circle cx="80" cy="50" r="34" fill="var(--dark)"/><circle cx="80" cy="50" r="43" fill="none"/><path d="M31 10L129 90M31 90L129 10" fill="none" stroke-width="5"/>'+emblem(school,80,50,.7)+rays(8)
 if i<30:
  return objects(UTILITY_OBJECTS[i-24],.75,20,8)+'<path d="M18 87L37 77M122 77L142 87M45 91H115" fill="none"/>'+emblem(school,132,24,.33)
 # Late two-school cards: two faces of the same end-of-round clock.
 return '<circle cx="80" cy="50" r="42"/><circle cx="80" cy="50" r="33" fill="var(--dark)"/><path d="M80 17V50L105 69M80 4V12M35 50H43M119 50H126M80 88V96" fill="none"/>'+emblem(schools[0],66,50,.48)+emblem(schools[1],98,50,.48)

def main():
 names=dict(line.split(' = ',1)for line in(ROOT/'content/i18n/pl.ftl').read_text(encoding='utf8').splitlines()if' = 'in line)
 for edition in ['relic','covenant','eclipse']:
  cards=tomllib.loads((ROOT/f'content/cards/{edition}/second_packet.toml').read_text(encoding='utf8'))['card'];gallery=[]
  for i,card in enumerate(cards):
   key=card['key'].removeprefix('card.');label=names[f'card-{key}-name'];dark,light=PALETTE[next(iter(card['requires']))]
   svg=f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 160 100" role="img" aria-label="{escape(label)}"><title>{escape(label)}</title><defs><radialGradient id="bg" cx="50%" cy="40%"><stop stop-color="{dark}"/><stop offset="1" stop-color="#15121f"/></radialGradient><linearGradient id="metal" x2="1" y2="1"><stop stop-color="#f4dfb8"/><stop offset=".5" stop-color="{light}"/><stop offset="1" stop-color="{dark}"/></linearGradient></defs><rect width="160" height="100" fill="url(#bg)"/><path d="M11 19L53 8M107 92L149 81" fill="none" stroke="{light}" opacity=".2"/><g fill="url(#metal)" stroke="{light}" stroke-width="1.7" stroke-linejoin="round" stroke-linecap="round" style="--dark:{dark}">{drawing(card,i)}</g><path d="M4 18V4H20M140 4H156V18M4 82V96H20M140 96H156V82" fill="none" stroke="#cfb580"/></svg>'''
   (ROOT/f'art/cards/{key}.svg').write_text(svg,encoding='utf8',newline='\n');gallery.append(f'<article><img width="240" height="150" src="cards/{key}.svg" alt="{escape(label)}"><h2>{escape(label)}</h2></article>')
  (ROOT/f'art/{edition}-second-review.html').write_text('<!doctype html><html lang="pl"><meta charset="utf-8"><title>'+edition+' — druga paczka</title><style>body{background:#17121c;color:#ead5a3;font:16px Georgia,serif;padding:24px}main{display:grid;grid-template-columns:repeat(4,1fr);gap:20px}h2{font-size:17px}img{width:100%;height:auto}</style><h1>'+edition+' — druga paczka</h1><main>'+''.join(gallery)+'</main></html>',encoding='utf8',newline='\n')
  print(f'{edition}: {len(cards)} illustrations')
if __name__=='__main__':main()
