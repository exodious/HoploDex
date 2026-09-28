# Seed photos

Real firearm photos for the human-testing seed (`../human_seed.rs`), so the
collection looks and feels like a real one. Each firearm that gets photos
takes them from here with `include_bytes!`. Most seeded firearms get no photos
at all, the way many people's collections look. Synthetic gradient images
are still used for size and count testing.

Every photo is public domain, CC0, CC BY or the UK Open Government Licence
v3.0. None are ShareAlike, NonCommercial or NoDerivatives, so they can also
appear in screenshots on a project web page. Attribution is required for the
CC BY and OGL files, and good practice for the rest, so any page showing them
should carry the credit lines in [Credits for a web page](#credits-for-a-web-page).

## How they were prepared

- All were found on and downloaded from Wikimedia Commons on 2026-09-27
  (UTC). The time of each download is in the per-file record below.
- Each download's SHA-1 was checked against the SHA-1 Commons reports for
  that file version (below), so the record pins the exact image used.
- **Changes made to every file:** scaled to fit within 1600 × 1600 pixels
  (files already smaller were not scaled), all metadata (EXIF, XMP, ICC,
  comments) stripped, and re-encoded as a progressive JPEG at quality 82 with
  ImageMagick:
  `magick <original> -auto-orient -resize '1600x1600>' -strip -sampling-factor 4:2:0 -quality 82 -interlace JPEG <out>.jpg`.
  Nothing was cropped, retouched or otherwise edited.
- Each was viewed at full size before use: no people, and no legible serial
  numbers. The "42045" on the M1 Garand's stock is the Smithsonian catalogue
  number (AF*42045), not a serial.
- Rejected on the way: two "CC0" background-removed PNGs on Commons (a SIG
  P365 and a P320 X-Carry) that are derivatives of other users' CC BY-SA
  photos, so the CC0 claim is doubtful; photos showing people; and
  manufacturer and retailer images.

To add a photo, follow the same steps and add an entry below, including the
Commons file version's SHA-1 and timestamp and the description page's
revision. Keep to public domain, CC0, CC BY or OGL, and keep a firearm's
photos to at most five.

## Which firearm uses which

| File | Seeded firearm | Licence |
| --- | --- | --- |
| `glock-19-gen4-fde.jpg` | Glock 19 Gen5 (3 photos) | CC0 1.0 |
| `glock-19-gen3.jpg` | Glock 19 Gen5 | Public domain (PD-self) |
| `glock-19-atf.jpg` | Glock 19 Gen5 | Public domain (US Government, ATF) |
| `m1-garand-left.jpg` | Springfield Armory M1 Garand (5 photos) | Public domain (Smithsonian) |
| `m1-garand-right.jpg` | Springfield Armory M1 Garand | Public domain (Smithsonian) |
| `m1-garand-receiver.jpg` | Springfield Armory M1 Garand | Public domain (Smithsonian) |
| `m1-garand-stock.jpg` | Springfield Armory M1 Garand | Public domain (Smithsonian) |
| `m1-garand-sling.jpg` | Springfield Armory M1 Garand | Public domain (Smithsonian) |
| `1911a1-field-stripped.jpg` | Colt 1911 Government Model (2 photos) | Public domain (PD-self) |
| `colt-m1911-markings.jpg` | Colt 1911 Government Model | CC BY 2.0 |
| `sw-686-cylinder.jpg` | Smith & Wesson Model 686 Plus (2 photos) | CC BY 3.0 |
| `sw-686-side.jpg` | Smith & Wesson Model 686 Plus (the chosen thumbnail) | CC BY 3.0 |
| `benelli-m4.jpg` | Benelli M4 Super 90 | OGL v3.0 |
| `beretta-92fs-atf.jpg` | Beretta 92FS (the imported one) | Public domain (US Government, ATF) |
| `remington-new-model-army.jpg` | Pedersoli 1858 Remington Replica | Public domain (PD-user) |
| `sig-p320-m18.jpg` | Sig Sauer P320 | CC BY 2.0 |
| `winchester-model-70-featherweight.jpg` | Winchester Model 70 Featherweight | CC BY 4.0 |

The photos show the model, not necessarily the exact variant seeded: the
Glock photos are Gen3 and Gen4, the 1911s are a Springfield Mil-Spec 1911A1
and a US-property Colt M1911, the P320 is the M18, the Benelli is the British
Army's L128A1, and the "Pedersoli replica" is an original Remington New Model
Army.

## Per-file record

"Commons file version" is the upload time and SHA-1 of the version of the
file that was downloaded. "Description page" is the revision of the Commons
page (licence and author) as read when checked, 2026-09-27T02:21:04Z.

### `glock-19-gen4-fde.jpg`

- Title: Glock 19 Generation 4 9mm Pistol
- Author: Arielnyc2006 (own work)
- Licence: CC0 1.0 Universal Public Domain Dedication, https://creativecommons.org/publicdomain/zero/1.0/
- Commons page: https://commons.wikimedia.org/wiki/File:Glock_19_Generation_4_9mm_Pistol.jpg
- Original file: https://upload.wikimedia.org/wikipedia/commons/6/6d/Glock_19_Generation_4_9mm_Pistol.jpg
- Taken: 2014-03-11
- Commons file version: 2014-03-12T03:13:50Z, 3648 × 2736, SHA-1 `62547cba7542a1afd6775d8ff01c116ceab6bd75`
- Description page: revision 1204923949 (2026-04-28T08:34:30Z)
- Retrieved: 2026-09-27T02:16:58Z
- Changes: scaled to 1600 × 1200, metadata stripped, re-encoded

### `glock-19-gen3.jpg`

- Title: Glock 19 (9mm), Generation 3
- Author: Agazoo (English Wikipedia user; source "personal collection")
- Licence: public domain, released by the author ({{PD-self}})
- Commons page: https://commons.wikimedia.org/wiki/File:Glock_19_(9mm),_Generation_3.jpg
- Original file: https://upload.wikimedia.org/wikipedia/commons/0/09/Glock_19_%289mm%29%2C_Generation_3.jpg
- Taken: 2012-03-21
- Commons file version: 2012-03-21T03:39:35Z, 1280 × 867, SHA-1 `fc629fa16077d1beef652134f9b1faff9368cd10`
- Description page: revision 1142923967 (2026-01-06T11:03:24Z)
- Retrieved: 2026-09-27T02:16:58Z
- Changes: not scaled; metadata stripped, re-encoded

### `glock-19-atf.jpg`

- Title: Glock 19 (transparent background)
- Author: U.S. Bureau of Alcohol, Tobacco, Firearms and Explosives
- Licence: public domain, a work of the US federal government ({{PD-USGov-DOJ}})
- Commons page: https://commons.wikimedia.org/wiki/File:Glock_19_(transparent_background).jpg
- Original file: https://upload.wikimedia.org/wikipedia/commons/d/d0/Glock_19_%28transparent_background%29.jpg
- Upstream source: https://www.atf.gov/n/n/n/sites/default/files/glock.jpg
- Dated: 2013-11-22
- Commons file version: 2018-06-30T09:06:33Z, 1200 × 600, SHA-1 `01adc27d72e62a0141c366d528e1056d73fb0cc1`
- Description page: revision 1274656301 (2026-09-10T08:09:57Z)
- Retrieved: 2026-09-27T02:16:58Z
- Changes: not scaled; metadata stripped, re-encoded

### `m1-garand-left.jpg`, `m1-garand-right.jpg`, `m1-garand-stock.jpg`, `m1-garand-receiver.jpg`, `m1-garand-sling.jpg`

Five photos of one rifle, object AF*42045 in the National Museum of American
History.

- Title: Springfield Armory M1 Garand Rifle
- Author: The Smithsonian Institution, National Museum of American History
- Licence: public domain ({{PD-USGov-SI}} on Commons)
- Upstream source: https://americanhistory.si.edu/collections/search/object/nmah_414892
- Changes: `-left`, `-right`, `-stock` and `-receiver` scaled to 1600 px wide; `-sling` scaled to 1600 × 937; all metadata stripped, re-encoded

| File | Commons file | Taken | Commons file version | SHA-1 | Description page | Retrieved |
| --- | --- | --- | --- | --- | --- | --- |
| `m1-garand-left.jpg` | [NMAH-AHB2015q026945](https://commons.wikimedia.org/wiki/File:Springfield_Armory_M1_Garand_Rifle-NMAH-AHB2015q026945.jpg) ([original](https://upload.wikimedia.org/wikipedia/commons/6/6b/Springfield_Armory_M1_Garand_Rifle-NMAH-AHB2015q026945.jpg)) | 2015-04-12 | 2021-04-02T04:20:53Z, 2000 × 650 | `bd1057ab94e47ddfb0cc1b2037396b9320635255` | rev 1092749132 (2025-09-29T17:46:01Z) | 2026-09-27T02:16:58Z |
| `m1-garand-right.jpg` | [NMAH-AHB2015q026946](https://commons.wikimedia.org/wiki/File:Springfield_Armory_M1_Garand_Rifle-NMAH-AHB2015q026946.jpg) ([original](https://upload.wikimedia.org/wikipedia/commons/1/11/Springfield_Armory_M1_Garand_Rifle-NMAH-AHB2015q026946.jpg)) | 2015-04-12 | 2021-04-02T04:22:04Z, 2000 × 650 | `e1fb1902ef3f166db7ec4b4189efb7b776c3b985` | rev 1152336481 (2026-01-24T08:41:35Z) | 2026-09-27T02:17:46Z |
| `m1-garand-stock.jpg` | [NMAH-AHB2015q026947](https://commons.wikimedia.org/wiki/File:Springfield_Armory_M1_Garand_Rifle-NMAH-AHB2015q026947.jpg) ([original](https://upload.wikimedia.org/wikipedia/commons/d/d0/Springfield_Armory_M1_Garand_Rifle-NMAH-AHB2015q026947.jpg)) | 2015-04-12 | 2021-04-02T04:13:20Z, 2000 × 1500 | `f3ada64fe9a75e61a364299d15f87438b5f7d9b1` | rev 549130948 (2021-04-02T04:13:20Z) | 2026-09-27T02:17:46Z |
| `m1-garand-receiver.jpg` | [NMAH-AHB2015q026948](https://commons.wikimedia.org/wiki/File:Springfield_Armory_M1_Garand_Rifle-NMAH-AHB2015q026948.jpg) ([original](https://upload.wikimedia.org/wikipedia/commons/c/c3/Springfield_Armory_M1_Garand_Rifle-NMAH-AHB2015q026948.jpg)) | 2015-04-12 | 2021-04-02T04:14:40Z, 2000 × 1500 | `8cbe00c5e97a2bba028438f51418eb7a942be23c` | rev 549181181 (2021-04-02T07:14:46Z) | 2026-09-27T02:17:46Z |
| `m1-garand-sling.jpg` | [NMAH-JN2020-00731](https://commons.wikimedia.org/wiki/File:Springfield_Armory_M1_Garand_Rifle-NMAH-JN2020-00731.jpg) ([original](https://upload.wikimedia.org/wikipedia/commons/4/42/Springfield_Armory_M1_Garand_Rifle-NMAH-JN2020-00731.jpg)) | 2019-03-21 | 2021-04-02T04:06:14Z, 1999 × 1171 | `d87e4f9a07ca126e76e1dc3d8603d4fb475a1022` | rev 549178644 (2021-04-02T07:06:23Z) | 2026-09-27T02:17:46Z |

### `1911a1-field-stripped.jpg`

- Title: 1911A1-JH02 ("Springfield 1911-A1 (Mil Spec) caliber 45 ACP, stripped")
- Author: Jan Hrdonka (English Wikipedia user Hrd10), own work
- Licence: public domain, released by the author ({{PD-self}})
- Commons page: https://commons.wikimedia.org/wiki/File:1911A1-JH02.jpg
- Original file: https://upload.wikimedia.org/wikipedia/commons/2/2f/1911A1-JH02.jpg
- Taken: 2007-08-20
- Commons file version: 2007-08-25T19:47:07Z, 2632 × 1868, SHA-1 `d7a2408beb76ea8dda120471604a9ec52b55f726`
- Description page: revision 1195994874 (2026-04-12T07:22:11Z)
- Retrieved: 2026-09-27T02:16:58Z
- Changes: scaled to 1600 × 1136, metadata stripped, re-encoded

### `colt-m1911-markings.jpg`

- Title: Colt M1911 detalj
- Author: Askild Antonsen (Flickr user handvapensamlingen, "Pistols used in Norway")
- Licence: CC BY 2.0, https://creativecommons.org/licenses/by/2.0/
- Commons page: https://commons.wikimedia.org/wiki/File:Colt_M1911_detalj_(6971799585).jpg
- Original file: https://upload.wikimedia.org/wikipedia/commons/3/39/Colt_M1911_detalj_%286971799585%29.jpg
- Upstream source: https://www.flickr.com/photos/handvapensamlingen/6971799585/
- Taken: 2012-01-28
- Commons file version: 2017-02-11T07:39:27Z, 4183 × 2502, SHA-1 `bcd54d989c286cd5a872cfdc46e63d6da7878f4d`
- Description page: revision 962616049 (2024-11-28T07:06:03Z)
- Retrieved: 2026-09-27T02:16:58Z
- Changes: scaled to 1600 × 957, metadata stripped, re-encoded

### `sw-686-cylinder.jpg`

- Title: SW 686 cylinder close-up ("Smith & Wesson 686 6" .357 Magnum, close-up of cylinder")
- Author: Spencer Dutch, own work
- Licence: CC BY 3.0, https://creativecommons.org/licenses/by/3.0/
- Commons page: https://commons.wikimedia.org/wiki/File:SW_686_cylinder_close-up.jpg
- Original file: https://upload.wikimedia.org/wikipedia/commons/f/fd/SW_686_cylinder_close-up.jpg
- Taken: 2009-10-04
- Commons file version: 2009-10-08T05:12:12Z, 1697 × 2544, SHA-1 `d2c095f66d255366f336d3a490c641d3d8b45266`
- Description page: revision 1084370565 (2025-09-12T01:43:34Z)
- Retrieved: 2026-09-27T02:16:58Z
- Changes: scaled to 1067 × 1600, metadata stripped, re-encoded

### `sw-686-side.jpg`

- Title: SW 686 side front view ("Smith & Wesson 686 6" .357 Magnum")
- Author: Spencer Dutch, own work
- Licence: CC BY 3.0, https://creativecommons.org/licenses/by/3.0/
- Commons page: https://commons.wikimedia.org/wiki/File:SW_686_side_front_view.jpg
- Original file: https://upload.wikimedia.org/wikipedia/commons/5/52/SW_686_side_front_view.jpg
- Taken: 2009-10-04
- Commons file version: 2009-10-08T05:08:12Z, 2314 × 1543, SHA-1 `5cd1e669060e26b57715994fd05bd78765aa647d`
- Description page: revision 1278070677 (2026-09-18T06:57:17Z)
- Retrieved: 2026-09-27T02:16:58Z
- Changes: scaled to 1600 × 1067, metadata stripped, re-encoded

### `benelli-m4.jpg`

- Title: L128A1 Combat Shotgun (Benelli M4) R004D172
- Author: Steve Dock, UK Ministry of Defence (Defence Imagery)
- Licence: Open Government Licence v3.0, https://www.nationalarchives.gov.uk/doc/open-government-licence/version/3/
- Credit line given on Commons: "Photo: Steve Dock/UK Ministry of Defence 2025"
- Commons page: https://commons.wikimedia.org/wiki/File:L128A1_Combat_Shotgun_(Benelli_M4)_R004D172.jpg
- Original file: https://upload.wikimedia.org/wikipedia/commons/1/16/L128A1_Combat_Shotgun_%28Benelli_M4%29_R004D172.jpg
- Upstream source: https://www.defenceimagery.mod.uk/Home/Search?Query=R004D172.jpg&Type=Filename
- Taken: 2014-01-17
- Commons file version: 2025-11-08T19:46:08Z, 5012 × 1920, SHA-1 `812bb71493da0568bf4edc675dcf65cec3236be8`
- Description page: revision 1115415961 (2025-11-14T13:38:49Z)
- Retrieved: 2026-09-27T02:20:26Z
- Changes: scaled to 1600 × 613, metadata stripped, re-encoded

### `beretta-92fs-atf.jpg`

- Title: Beretta 92FS hand gun
- Author: U.S. Bureau of Alcohol, Tobacco, Firearms and Explosives
- Licence: public domain, a work of the US federal government ({{PD-USGov-DOJ}})
- Commons page: https://commons.wikimedia.org/wiki/File:Beretta_92FS_hand_gun.jpg
- Original file: https://upload.wikimedia.org/wikipedia/commons/2/26/Beretta_92FS_hand_gun.jpg
- Upstream source: https://www.atf.gov/n/n/n/sites/default/files/barrett_hand_gun_2.jpg
- Dated: 2013-11-22
- Commons file version: 2025-08-17T16:17:36Z, 1865 × 1320, SHA-1 `6a0b12c76660b5b95bca6512d9225c90cd0e2f70`
- Description page: revision 1145837941 (2026-01-11T03:43:09Z)
- Retrieved: 2026-09-27T02:16:58Z
- Changes: scaled to 1600 × 1132, metadata stripped, re-encoded

### `remington-new-model-army.jpg`

- Title: Remington New Model Army Revolver
- Author: Hmaag (German Wikipedia user), self-photographed
- Licence: public domain, released by the author ({{PD-user-de|Hmaag}})
- Commons page: https://commons.wikimedia.org/wiki/File:Remington_New_Model_Army_Revolver.JPG
- Original file: https://upload.wikimedia.org/wikipedia/commons/e/ea/Remington_New_Model_Army_Revolver.JPG
- First uploaded: 2009-01-06 (German Wikipedia)
- Commons file version: 2011-01-12T19:31:22Z, 1768 × 720, SHA-1 `ed553d531bc05e64cc7e0fef09b61b118f83d3e9`
- Description page: revision 1266614567 (2026-08-24T11:35:33Z)
- Retrieved: 2026-09-27T02:16:58Z
- Changes: scaled to 1600 × 652, metadata stripped, re-encoded

### `sig-p320-m18.jpg`

- Title: SIG Sauer P320 M18 Handgun Tan
- Author: Tony Webster (Flickr user diversey)
- Licence: CC BY 2.0, https://creativecommons.org/licenses/by/2.0/ (Commons' FlickreviewR bot confirmed the Flickr licence on 2025-02-27)
- Commons page: https://commons.wikimedia.org/wiki/File:SIG_Sauer_P320_M18_Handgun_Tan_(53745479381).jpg
- Original file: https://upload.wikimedia.org/wikipedia/commons/b/be/SIG_Sauer_P320_M18_Handgun_Tan_%2853745479381%29.jpg
- Upstream source: https://www.flickr.com/photos/diversey/53745479381/
- Taken: 2024-05-15
- Commons file version: 2025-02-27T03:28:46Z, 8176 × 5471, SHA-1 `e8dfac7ffc75db7298c74ac7eb1b742db20e9fa1`
- Description page: revision 1209645787 (2026-05-06T21:49:03Z)
- Retrieved: 2026-09-27T02:16:58Z
- Changes: scaled to 1600 × 1071, metadata stripped, re-encoded

### `winchester-model-70-featherweight.jpg`

- Title: Winchester Model 70 Classic Featherweight .300 WSM
- Author: Efeesh, own work
- Licence: CC BY 4.0, https://creativecommons.org/licenses/by/4.0/
- Commons page: https://commons.wikimedia.org/wiki/File:Winchester_Model_70_Classic_Featherweight_.300_WSM.jpg
- Original file: https://upload.wikimedia.org/wikipedia/commons/3/3f/Winchester_Model_70_Classic_Featherweight_.300_WSM.jpg
- Taken: 2026-04-05
- Commons file version: 2026-04-05T08:29:12Z, 3817 × 936, SHA-1 `4dee3b6537054f3149f366ab087a8465c0c2e07e`
- Description page: revision 1230509352 (2026-06-13T07:13:44Z)
- Retrieved: 2026-09-27T02:17:46Z
- Changes: scaled to 1600 × 392, metadata stripped, re-encoded

## Credits for a web page

Ready to paste under screenshots that show these photos. The CC BY and OGL
licences ask for changes to be noted, so those lines say "resized".

- Glock 19 Gen4 photo by Arielnyc2006, CC0, via Wikimedia Commons.
- Glock 19 Gen3 photo by Agazoo, public domain, via Wikimedia Commons.
- Glock 19 and Beretta 92FS photos by the U.S. Bureau of Alcohol, Tobacco, Firearms and Explosives, public domain, via Wikimedia Commons.
- Springfield Armory M1 Garand photos: Smithsonian Institution, National Museum of American History, public domain, via Wikimedia Commons.
- Springfield 1911A1 photo by Jan Hrdonka, public domain, via Wikimedia Commons.
- "Colt M1911 detalj" by Askild Antonsen, CC BY 2.0 (https://creativecommons.org/licenses/by/2.0/), via Wikimedia Commons; resized.
- Smith & Wesson 686 photos by Spencer Dutch, CC BY 3.0 (https://creativecommons.org/licenses/by/3.0/), via Wikimedia Commons; resized.
- L128A1 Combat Shotgun (Benelli M4) photo by Steve Dock/UK Ministry of Defence. Contains public sector information licensed under the Open Government Licence v3.0 (https://www.nationalarchives.gov.uk/doc/open-government-licence/version/3/); resized.
- Remington New Model Army photo by Hmaag, public domain, via Wikimedia Commons.
- "SIG Sauer P320 M18 Handgun Tan" by Tony Webster, CC BY 2.0 (https://creativecommons.org/licenses/by/2.0/), via Wikimedia Commons; resized.
- Winchester Model 70 Classic Featherweight photo by Efeesh, CC BY 4.0 (https://creativecommons.org/licenses/by/4.0/), via Wikimedia Commons; resized.
