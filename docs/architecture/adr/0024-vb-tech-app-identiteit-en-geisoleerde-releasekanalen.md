# ADR-0024: VB Tech-appidentiteit en geïsoleerde releasekanalen

- Status: Accepted
- Datum: 2026-08-08

## Context

Lumi moet als open-source macOS-app kunnen worden ontwikkeld en getest zonder
dat een development- of release-candidatebuild de waardevolle gebruikersdata
van de stabiele installatie opent of wijzigt. De eerdere bundle-identiteit
verwees bovendien naar een private naam die niet langer bij het publieke
software-engineeringwerk hoort.

Alle kanalen delen bewust dezelfde broncode en databaseschema's. De scheiding
is een operationele veiligheidsgrens, geen productfork.

## Besluit

Lumi gebruikt vanaf nu de publieke VB Tech-namespace `co.victorblan.tech`. De
reverse-DNS-vorm is de gebruikelijke Apple-conventie voor wereldwijd unieke
bundle identifiers; eigendom van de domeinnaam maakt de identifier herkenbaar
en beheersbaar.

Er zijn drie naast elkaar installeerbare macOS-kanalen:

| Kanaal | Appnaam | Bundle identifier | Application Support |
|---|---|---|---|
| Release | Lumi | `co.victorblan.tech.lumi` | `Lumi` |
| RC | Lumi | `co.victorblan.tech.lumi.rc` | `Lumi RC` (intern) |
| Dev | Lumi | `co.victorblan.tech.lumi.dev` | `Lumi Dev` (bestaand intern pad) |

De Xcode-configuraties zijn uitsluitend `Release`, `RC` en `Dev`.
Iedere appbundle declareert zijn kanaal en datadirectory in `Info.plist`; de app
leidt het databasepad uitsluitend daaruit af. Preferences worden door de
verschillende bundle identifiers in afzonderlijke macOS-domeinen bewaard.

De bestaande `Application Support/Lumi/library.sqlite` blijft de releasedata.
Een expliciet lokaal hulpmiddel mag met SQLite's online backup-API een eenmalige
kopie naar RC of Dev maken. Het overschrijft nooit een bestaand doel. Een
apart back-uphulpmiddel valideert de kopie met `PRAGMA integrity_check`.

RC is het kanaal voor lokaal verpakte acceptatie-DMG's. Release wordt uitsluitend
vanaf een bewust gepromoveerde versie gebouwd. Dev is de normale lokale
ontwikkelconfiguratie. De appnaam is in elk kanaal `Lumi`; uitsluitend de
zichtbare productversie (`dev-N`, `rc-N` of zonder suffix) onderscheidt builds.

## Gevolgen

- Dev-, RC- en releasedatabases kunnen onafhankelijk migreren en getest
  worden zonder impliciete kruisbestuiving.
- Een handmatige wijziging in één kanaal verschijnt niet automatisch in een
  ander kanaal; opnieuw klonen is een expliciete actie.
- De apps kunnen naast elkaar geïnstalleerd zijn. Voor live MIDI- en
  lichtaansturing hoort maar één Lumi-runtime tegelijk actief te zijn totdat
  endpoints ook per kanaal geïsoleerd zijn.
- Historische commits mogen de eerdere identifier behouden; actieve broncode,
  binaries en documentatie gebruiken die identiteit niet meer.
