# Glossary

The terms of the interface in each of the app's languages (E15), so that every translation of
`resources/<lang>.lproj/Localizable.strings` uses one word for one thing. The keys are the German texts in the code;
`en.lproj` gives the English meaning. Each language below has its own table; a translation that departs from it
changes the table first.

## Rules for every language
- **Placeholders:** every `{}` of the key stays, as many as the key has and in the same order, since the code fills
  them one after the other. Where the language wants another order, the sentence is rephrased.
- **Symbols and names:** `⌘ ⌥ ⇧ ⌃ ↩ ⎋`, key names such as Esc, and file names, commands and flags stay as they are.
  Product and service names are never translated: Claude, Codex, Gemini, Antigravity, Anthropic, OpenAI, OpenRouter,
  Ollama, LM Studio, Lowspeak, Git, GitHub, GitLab, ACP, MCP, CI, OAuth, PATH, JSON, Finder, macOS, Mac, claude.ai,
  Remote Control (Anthropic's feature of that name).
- **The app's name** never appears in a translation; where a key names the app, it has a `{}` for it.
- **macOS first:** menu items, buttons and system words that macOS has (the Standard terms below) take the wording
  of macOS in that language, letter for letter, including how it writes the ellipsis of an item that opens a
  dialog and whether a space goes before it.
- **Address:** as macOS addresses the user in that language; the German keys say "du" because German macOS does.
- **Typography:** the quotation marks, apostrophes and number formats of the language as macOS uses them.
- **Length:** buttons, menu items, column titles and tab names stay about as short as the English; explanations may
  run longer.
- **Case:** the casing macOS uses in that language; title case in menus is an English habit only.
- **Escapes in `.strings`:** `\"` for a quote inside a text, `\\` for a backslash, `\n` for a line break, as many line
  breaks as the English.
- **Reuse:** a key that splat (`../splat/resources`) already translates takes splat's translation, so both apps say
  the same, unless splat's text means something else here or departs from macOS. So far that is the case for
  Zeilenumbruch (here Wrap Lines), Neu (the diff's column of new lines), Weiter (Continue), Übernehmen (Use Key) and
  Hinweis (the language server's severity Hint) in every language, Im Finder zeigen in ar, ca, ro, sl and zh-Hant,
  Rückwärts suchen in sk, Vollbildmodus in vi and Zoomen in ja.

## Terms
What each term means in this app. The Standard terms are macOS's own; the others are the app's.

| German key | English | Meaning |
|---|---|---|
| **Standard** | | |
| Ablage | File | the menu |
| Bearbeiten | Edit | the menu |
| Darstellung | View | the menu |
| Fenster | Window | the menu |
| Hilfe | Help | the menu |
| Dienste | Services | the submenu of the app menu |
| Über {} | About {} | app menu |
| Einstellungen | Settings | app menu and the settings window |
| {} ausblenden / Andere ausblenden / Alle einblenden | Hide {} / Hide Others / Show All | app menu |
| {} beenden | Quit {} | app menu |
| Widerrufen / Wiederholen | Undo / Redo | Edit menu |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Cut / Copy / Paste / Delete | Edit menu |
| Alles auswählen | Select All | Edit menu |
| Suchen … / Weitersuchen / Rückwärts suchen | Find… / Find Next / Find Previous | Find submenu |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Minimize / Zoom / Bring All to Front | Window menu |
| Vollbildmodus | Enter Full Screen | View menu |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Hide Toolbar / Toggle Sidebar | View menu |
| Sichern | Save | write a file (macOS German says "Sichern") |
| Öffnen / Schließen | Open / Close | |
| Abbrechen / OK | Cancel / OK | buttons |
| Im Finder zeigen | Show in Finder | |
| Papierkorb | Trash | |
| Schlüsselbund | Keychain | where keys and tokens are kept |
| Zwischenablage | Clipboard | |
| **Window and editor** | | |
| Tab | tab | one document, terminal, browser page or AI thread in a panel |
| AI-Tab | AI tab | the tab of an agent's thread |
| Panel | panel | a part of the window beside the editor, such as the Git panel |
| Seitenleiste | sidebar | files, agents, Git, CI on the left |
| Vorschau | preview | a tab opened by a single click, replaced by the next one; also the rendered view of a Markdown, diagram or PDF file |
| Aktivität | Activity | the list of running processes, sessions and tasks |
| Projekt / Ordner | project / folder | the folder a window shows |
| Vertrauen, vertrauen | trust | the user allows a folder to run what it configures |
| Platte | disk | "on disk": the file as stored, against the editor's text |
| Unterschiede | differences | a diff |
| Zusammenführen | merge | of changes, also Git's merge |
| Konflikt | conflict | |
| Treffer | match | of a search |
| Suche | search | the sidebar panel that searches the project |
| Ersetzen | replace | in the search panel and in a file |
| Sprachserver | language server | LSP |
| Hinweis (Schweregrad) | hint | the language server's lowest severity, after Fehler, Warnung and Info |
| Lesezeichen / Verlauf / Adresse | bookmark / history / address | of the browser tab |
| Bytes | bytes | the unit |
| Exportieren | export | the submenu that writes Markdown as PDF, HTML or Word |
| Format | format | the menu of Markdown formatting |
| Diktat | dictation | speech to text |
| **Agents** | | |
| Agent | agent | a coding agent such as Claude, Codex or Gemini |
| Eigener Agent | own agent | the agent built into the app |
| Eigene Agenten | custom agents | the subagents a user defines for Claude, not the own agent |
| Thread (AI-Thread, CLI-Thread) | thread | one conversation with an agent as the app shows it |
| Sitzung | session | a running conversation with an agent's process |
| Unterhaltung | conversation | the messages of a thread |
| Zug | turn | one message of the user and the agent's work on it |
| Antwort | answer | the agent's reply |
| Rückfrage | question | the agent asks the user |
| Freigabe, Freigabemodus | permission, permission mode | the agent asks before it uses a tool |
| Umgehen | bypass | the permission mode that never asks |
| Erlauben / Ablehnen / Immer erlauben … | Allow / Deny / Always Allow… | answers to a permission |
| Plan | plan | the agent's plan before it acts |
| To-dos | to-dos | the agent's to-do list |
| Aufgabe | task | work an agent runs in the background |
| Auftrag | assignment | what a subagent was asked to do; not Aufgabe |
| Werkzeug | tool | what an agent may use: shell, files, web, MCP |
| Skills | skills | Claude's skills |
| Befehl | command | a shell or slash command |
| Hook | hook | Claude's and Git's hooks |
| Sandbox | sandbox | the limits an agent runs in |
| Fremder Inhalt | external content | text from a web page, a terminal, a CI log or a file outside the project, given to the agent as data |
| Sicherungspunkt | checkpoint | the project's state taken before each prompt |
| Zurückspulen | rewind | back to a checkpoint or an earlier message |
| Verdichten | compact | shorten the conversation to fit the context |
| Kontext, Kontextfenster | context, context window | |
| Denktiefe | effort | how long the model thinks |
| Modell / Anbieter / Endpunkt | model / provider / endpoint | |
| Token, Tokens | token, tokens | |
| API-Schlüssel | API key | |
| Register | registry | the ACP registry of agents |
| Adapter | adapter | the program that connects an agent's CLI over ACP |
| Bereit | ready | an agent whose program is there and can start |
| Prüfsumme | checksum | |
| Anmeldung, anmelden | sign-in, sign in | with an agent's or a forge's service |
| Konto | account | |
| Abo | subscription | a paid plan of an agent's service, used up against its limits rather than per token |
| **Git and CI** | | |
| Repository | repository | |
| Commit, committen | commit | |
| Secrets | secrets | passwords and tokens the secrets check finds before a commit |
| vormerken | stage | |
| Zurücknehmen | unstage | the opposite of vormerken |
| nicht versioniert | untracked | |
| Abschnitt (Git) | hunk | a block of changed lines |
| Branch / Remote | branch / remote | |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase | Git operations |
| Worktree | worktree | a second working copy for a thread |
| Übernehmen (Worktree) | take over | bring a worktree thread's work into the user's branch |
| Arbeitskopie | working tree | |
| Lauf, Läufe | run, runs | of CI |
| Host | host | the server of a CI service, such as a self-managed GitLab |
| Job | job | of CI |
| Log | log | |

## Languages

### Arabic (`ar`)
- Address: second person singular in the generic masculine form, as macOS does ("هل تريد…؟", "اختر", "أدخل"); menu items
  and buttons are verbal nouns ("فتح", "حفظ", "إظهار الشريط الجانبي")
- Typography: “…” quotation marks (U+201C/U+201D in logical order, as macOS's Arabic texts write them); Arabic comma
  "،", semicolon "؛" and question mark "؟"; the ellipsis of an item that opens a dialog follows the word without a space
  ("الإعدادات…", "فتح…"); Western digits; a right-to-left mark (U+200F) opens a label that would otherwise begin with a
  placeholder or Latin text, and a left-to-right mark (U+200E) keeps a leading or trailing dot of a file name or command
  on its side; Latin names and commands stay in Latin script inside the right-to-left text
- Git and CI words: Arabic for the concepts that have settled Arabic words (مستودع repository, فرع branch, المستودع
  البعيد remote, دمج merge, تعارض conflict, تجهيز stage, مقطع hunk, نسخة العمل working tree, غير متتبَّع untracked,
  تشغيل run, وظيفة job, سجل log, ضم for taking a worktree's work over); the Git operations stay as Latin loanwords as
  Arabic developers say and type them (Commit, Push, Pull, Fetch, Rebase, Revert, Cherry-Pick, Upstream, Worktree), with
  an Arabic verbal noun where a sentence needs a verb ("إنشاء Commit", "تنفيذ Push", plural "عمليات Commit"); "AI" stays
  in Latin, as the German key keeps it ("سلسلة AI")

| German key | Arabic |
|---|---|
| Ablage | ملف |
| Bearbeiten | تعديل |
| Darstellung | عرض |
| Fenster | نافذة |
| Hilfe | مساعدة |
| Dienste | خدمات |
| Über {} | حول {} |
| Einstellungen | الإعدادات |
| {} ausblenden / Andere ausblenden / Alle einblenden | إخفاء {} / إخفاء الأخرى / إظهار الكل |
| {} beenden | إنهاء {} |
| Widerrufen / Wiederholen | تراجع / إعادة |
| Ausschneiden / Kopieren / Einsetzen / Löschen | قص / نسخ / لصق / حذف |
| Alles auswählen | تحديد الكل |
| Suchen … / Weitersuchen / Rückwärts suchen | بحث… / بحث عن التالي / بحث عن السابق |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | تصغير / تكبير/تصغير / إحضار الكل إلى الأمام |
| Vollbildmodus | الدخول إلى وضع ملء الشاشة |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | إخفاء شريط الأدوات / إظهار الشريط الجانبي أو إخفاؤه |
| Sichern | حفظ |
| Öffnen / Schließen | فتح / إغلاق |
| Abbrechen / OK | إلغاء / موافق |
| Im Finder zeigen | إظهار في Finder |
| Papierkorb | سلة المهملات |
| Schlüsselbund | سلسلة المفاتيح |
| Zwischenablage | الحافظة |
| Tab | علامة تبويب |
| AI-Tab | علامة تبويب AI |
| Panel | لوحة |
| Seitenleiste | الشريط الجانبي |
| Vorschau | معاينة |
| Aktivität | النشاط |
| Projekt / Ordner | مشروع / مجلد |
| Vertrauen, vertrauen | الثقة، الوثوق (يثق) |
| Platte | القرص ("على القرص") |
| Unterschiede | الفروقات |
| Zusammenführen | دمج |
| Konflikt | تعارض |
| Treffer | نتيجة، نتائج |
| Suche | بحث |
| Ersetzen | استبدال |
| Sprachserver | خادم اللغة |
| Hinweis (Schweregrad) | تلميح |
| Lesezeichen / Verlauf / Adresse | إشارة مرجعية / السجل / العنوان |
| Bytes | بايت |
| Exportieren | تصدير |
| Format | التنسيق |
| Diktat | الإملاء |
| Agent | وكيل |
| Eigener Agent | الوكيل المدمج |
| Eigene Agenten | الوكلاء المخصصون |
| Thread (AI-Thread, CLI-Thread) | سلسلة (سلسلة AI، سلسلة CLI) |
| Sitzung | جلسة |
| Unterhaltung | محادثة |
| Zug | دور |
| Antwort | رد |
| Rückfrage | سؤال |
| Freigabe, Freigabemodus | إذن، وضع الأذونات |
| Umgehen | تجاوز |
| Erlauben / Ablehnen / Immer erlauben … | سماح / رفض / السماح دائمًا… |
| Plan | خطة |
| To-dos | قائمة المهام |
| Aufgabe | مهمة |
| Auftrag | التكليف |
| Werkzeug | أداة |
| Skills | المهارات |
| Befehl | أمر |
| Hook | خطاف |
| Sandbox | بيئة معزولة |
| Fremder Inhalt | محتوى خارجي |
| Sicherungspunkt | نقطة استعادة |
| Zurückspulen | إرجاع |
| Verdichten | ضغط |
| Kontext, Kontextfenster | السياق، نافذة السياق |
| Denktiefe | عمق التفكير |
| Modell / Anbieter / Endpunkt | نموذج / مزوّد / نقطة نهاية |
| Token, Tokens | رمز مميز، رموز مميزة |
| API-Schlüssel | مفتاح API |
| Register | فهرس (فهرس ACP) |
| Adapter | المحوّل |
| Bereit | جاهز |
| Prüfsumme | المجموع الاختباري |
| Anmeldung, anmelden | تسجيل الدخول |
| Konto | حساب |
| Abo | الاشتراك |
| Repository | مستودع |
| Commit, committen | Commit، إنشاء Commit |
| Secrets | بيانات سرية |
| vormerken | تجهيز |
| Zurücknehmen | إلغاء التجهيز |
| nicht versioniert | غير متتبَّع |
| Abschnitt (Git) | مقطع |
| Branch / Remote | فرع / المستودع البعيد |
| Push / Pull / Fetch / Rebase | Push / Pull / Fetch / Rebase |
| Worktree | Worktree |
| Übernehmen (Worktree) | ضم |
| Arbeitskopie | نسخة العمل |
| Lauf, Läufe | تشغيل، عمليات التشغيل |
| Host | مضيف |
| Job | وظيفة |
| Log | سجل |

### Catalan (`ca`)
- Address: informal "tu", as macOS Catalan does ("Vols desar els canvis?", "Torna‑ho a provar"); menu items and buttons
  in the imperative ("Desa", "Obre", "Cancel·la")
- Typography: “…” curly double quotes as macOS Catalan uses them (not « »), typographic apostrophe ’, "l·l" with the
  middle dot, clitic pronouns joined with the non-breaking hyphen as in "Mostra‑ho tot"; the ellipsis follows the word
  without a space ("Cerca…", "Configuració…"); a space before % ("50 %")
- Git and CI words: kept as loanwords: commit, push, pull, fetch, rebase, cherry-pick, hook, worktree, job, log, token,
  sandbox ("fer commit", "fer push", "Fent pull…"); translated: repositori, branca, remot, fusionar (merge), preparar
  (stage), fragment (hunk), sense seguiment (untracked), còpia de treball, execució (CI run); "AI" is "IA" ("pestanya
  d’IA"); "file" is "fitxer" as splat's "Fitxers" has it

| German key | Catalan |
|---|---|
| Ablage | Arxiu |
| Bearbeiten | Edita |
| Darstellung | Mostra |
| Fenster | Finestra |
| Hilfe | Ajuda |
| Dienste | Serveis |
| Über {} | Sobre el {} |
| Einstellungen | Configuració |
| {} ausblenden / Andere ausblenden / Alle einblenden | Oculta el {} / Oculta la resta / Mostra‑ho tot |
| {} beenden | Surt del {} |
| Widerrufen / Wiederholen | Desfés / Refés |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Retalla / Copia / Enganxa / Elimina |
| Alles auswählen | Selecciona‑ho tot |
| Suchen … / Weitersuchen / Rückwärts suchen | Cerca… / Cerca el següent / Cerca l’anterior |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Minimitza / Zoom / Porta‑ho tot a primer pla |
| Vollbildmodus | Passa a pantalla completa |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Oculta la barra d’eines / Mostra o oculta la barra lateral |
| Sichern | Desa |
| Öffnen / Schließen | Obre / Tanca |
| Abbrechen / OK | Cancel·la / D’acord |
| Im Finder zeigen | Mostra al Finder |
| Papierkorb | paperera |
| Schlüsselbund | clauer |
| Zwischenablage | porta-retalls |
| Tab | pestanya |
| AI-Tab | pestanya d’IA |
| Panel | tauler |
| Seitenleiste | barra lateral |
| Vorschau | previsualització |
| Aktivität | Activitat |
| Projekt / Ordner | projecte / carpeta |
| Vertrauen, vertrauen | confiança, confiar (“Confia‑hi”) |
| Platte | disc (“al disc”) |
| Unterschiede | diferències |
| Zusammenführen | fusionar (“Fusiona”) |
| Konflikt | conflicte |
| Treffer | resultat |
| Suche | Cerca |
| Ersetzen | Substitueix |
| Sprachserver | servidor de llenguatge |
| Hinweis (Schweregrad) | Suggeriment |
| Lesezeichen / Verlauf / Adresse | marcador / historial / adreça |
| Bytes | bytes |
| Exportieren | Exporta |
| Format | Format |
| Diktat | dictat |
| Agent | agent |
| Eigener Agent | agent propi |
| Eigene Agenten | Agents personalitzats |
| Thread (AI-Thread, CLI-Thread) | fil (fil d’IA, fil de CLI) |
| Sitzung | sessió |
| Unterhaltung | conversa |
| Zug | torn |
| Antwort | resposta |
| Rückfrage | pregunta |
| Freigabe, Freigabemodus | permís, mode de permisos |
| Umgehen | Omet els permisos |
| Erlauben / Ablehnen / Immer erlauben … | Permet / Denega / Permet sempre… |
| Plan | pla |
| To-dos | Tasques pendents |
| Aufgabe | tasca |
| Auftrag | encàrrec |
| Werkzeug | eina |
| Skills | Habilitats |
| Befehl | ordre |
| Hook | hook |
| Sandbox | sandbox |
| Fremder Inhalt | Contingut extern |
| Sicherungspunkt | punt de control |
| Zurückspulen | retrocedir (“Retrocedeix”) |
| Verdichten | compactar |
| Kontext, Kontextfenster | context, finestra de context |
| Denktiefe | raonament |
| Modell / Anbieter / Endpunkt | model / proveïdor / punt de connexió |
| Token, Tokens | token, tokens |
| API-Schlüssel | clau d’API |
| Register | registre |
| Adapter | adaptador |
| Bereit | A punt |
| Prüfsumme | suma de verificació |
| Anmeldung, anmelden | inici de sessió, iniciar la sessió |
| Konto | compte |
| Abo | Subscripció |
| Repository | repositori |
| Commit, committen | commit, fer commit |
| Secrets | secrets |
| vormerken | preparar |
| Zurücknehmen | Treu de la preparació |
| nicht versioniert | sense seguiment |
| Abschnitt (Git) | fragment |
| Branch / Remote | branca / remot |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Übernehmen (Worktree) | incorporar |
| Arbeitskopie | còpia de treball |
| Lauf, Läufe | execució, execucions |
| Host | host |
| Job | job |
| Log | log |

### Czech (`cs`)
- Address: formal "vy" (vykání) with plural imperatives ("Zadejte", "Vyberte"), as macOS Czech; menu items and buttons
  are infinitives ("Otevřít", "Uložit"), help tags third person ("Vloží obrázek")
- Typography: „…“ (U+201E, U+201C), ‚…‘ inside; menu items end in "…" without a space ("Hledat…", "Přejmenovat…");
  sentence case; "50 %" with a space, decimal comma
- Git and CI words: kept as Czech developers say them: commit (verb "commitnout"), push, pull, fetch, rebase,
  cherry-pick, revert, fast-forward, hook, worktree, job, log, token; translated where Czech has the established word:
  větev (branch), vzdálený repozitář (remote), repozitář, sloučit/sloučení (merge), připravit k commitu (stage), blok
  (hunk), pracovní kopie, běh (CI run)

| German key | Czech |
|---|---|
| Ablage | Soubor |
| Bearbeiten | Úpravy |
| Darstellung | Zobrazení |
| Fenster | Okno |
| Hilfe | Nápověda |
| Dienste | Služby |
| Über {} | O aplikaci {} |
| Einstellungen | Nastavení |
| {} ausblenden / Andere ausblenden / Alle einblenden | Skrýt {} / Skrýt ostatní / Zobrazit vše |
| {} beenden | Ukončit {} |
| Widerrufen / Wiederholen | Odvolat akci / Opakovat akci |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Vyjmout / Zkopírovat / Vložit / Smazat |
| Alles auswählen | Vybrat vše |
| Suchen … / Weitersuchen / Rückwärts suchen | Hledat… / Hledat další / Hledat předchozí |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Minimalizovat / Přepnout velikost / Převést vše do popředí |
| Vollbildmodus | Spustit režim celé obrazovky |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Skrýt panel nástrojů / Zobrazit/skrýt boční panel |
| Sichern | Uložit |
| Öffnen / Schließen | Otevřít / Zavřít |
| Abbrechen / OK | Zrušit / OK |
| Im Finder zeigen | Zobrazit ve Finderu |
| Papierkorb | Koš |
| Schlüsselbund | Klíčenka |
| Zwischenablage | Schránka |
| Tab | panel |
| AI-Tab | panel AI |
| Panel | podokno |
| Seitenleiste | boční panel |
| Vorschau | náhled |
| Aktivität | Aktivita |
| Projekt / Ordner | projekt / složka |
| Vertrauen, vertrauen | důvěra, důvěřovat |
| Platte | disk |
| Unterschiede | rozdíly |
| Zusammenführen | sloučit, sloučení |
| Konflikt | konflikt |
| Treffer | výsledek |
| Suche | Hledání |
| Ersetzen | Nahradit |
| Sprachserver | jazykový server |
| Hinweis (Schweregrad) | Nápověda |
| Lesezeichen / Verlauf / Adresse | záložka / historie / adresa |
| Bytes | B |
| Exportieren | Exportovat |
| Format | Formát |
| Diktat | diktování |
| Agent | agent |
| Eigener Agent | vlastní agent |
| Eigene Agenten | Vlastní agenti |
| Thread (AI-Thread, CLI-Thread) | vlákno (vlákno AI, vlákno CLI) |
| Sitzung | relace |
| Unterhaltung | konverzace |
| Zug | kolo |
| Antwort | odpověď |
| Rückfrage | dotaz |
| Freigabe, Freigabemodus | povolení, režim povolení |
| Umgehen | Obejít |
| Erlauben / Ablehnen / Immer erlauben … | Povolit / Zamítnout / Vždy povolit… |
| Plan | plán |
| To-dos | Úkoly |
| Aufgabe | úloha |
| Auftrag | zadání |
| Werkzeug | nástroj |
| Skills | Dovednosti |
| Befehl | příkaz |
| Hook | hook |
| Sandbox | sandbox |
| Fremder Inhalt | Cizí obsah |
| Sicherungspunkt | kontrolní bod |
| Zurückspulen | přetočit zpět |
| Verdichten | zestručnit |
| Kontext, Kontextfenster | kontext, kontextové okno |
| Denktiefe | hloubka uvažování |
| Modell / Anbieter / Endpunkt | model / poskytovatel / koncový bod |
| Token, Tokens | token, tokeny |
| API-Schlüssel | klíč API |
| Register | registr |
| Adapter | adaptér |
| Bereit | Připraveno |
| Prüfsumme | kontrolní součet |
| Anmeldung, anmelden | přihlášení, přihlásit se |
| Konto | účet |
| Abo | Předplatné |
| Repository | repozitář |
| Commit, committen | commit, commitnout |
| Secrets | tajné údaje |
| vormerken | připravit k commitu |
| Zurücknehmen | Zrušit přípravu |
| nicht versioniert | nesledováno |
| Abschnitt (Git) | blok |
| Branch / Remote | větev / vzdálený repozitář |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Übernehmen (Worktree) | převzít |
| Arbeitskopie | pracovní kopie |
| Lauf, Läufe | běh, běhy |
| Host | hostitel |
| Job | job |
| Log | log |

### Danish (`da`)
- Address: informal "du", as Danish macOS addresses the user
- Typography: “…” quotation marks (as splat and Danish macOS), "Indstillinger…" with the ellipsis directly after the
  word, no space; a file is an "arkiv" as in Danish macOS and splat
- Git and CI words: kept as loanwords, as Danish developers say them: repository, commit/committe, branch, remote, push,
  pull, fetch, merge ("merget"), rebase, cherry-pick, stage/unstage ("staget", "ikke staget"), worktree, hook, token,
  job, log, secret; translated: arbejdskopi (working tree), blok (hunk), kørsel (CI run), vært (host)

| German key | Danish |
|---|---|
| Ablage | Arkiv |
| Bearbeiten | Rediger |
| Darstellung | Oversigt |
| Fenster | Vindue |
| Hilfe | Hjælp |
| Dienste | Tjenester |
| Über {} | Om {} |
| Einstellungen | Indstillinger |
| {} ausblenden / Andere ausblenden / Alle einblenden | Skjul {} / Skjul andre / Vis alle |
| {} beenden | Slut {} |
| Widerrufen / Wiederholen | Fortryd / Gentag |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Klip / Kopier / Sæt ind / Slet |
| Alles auswählen | Vælg alt |
| Suchen … / Weitersuchen / Rückwärts suchen | Find… / Find næste / Find forrige |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Minimer / Zoom / Anbring alle forrest |
| Vollbildmodus | Start fuld skærm |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Skjul værktøjslinje / Vis/skjul indholdsoversigt |
| Sichern | Gem |
| Öffnen / Schließen | Åbn / Luk |
| Abbrechen / OK | Annuller / OK |
| Im Finder zeigen | Vis i Finder |
| Papierkorb | papirkurv (Flyt til papirkurven) |
| Schlüsselbund | nøglering |
| Zwischenablage | udklipsholder |
| Tab | fane |
| AI-Tab | AI-fane |
| Panel | panel |
| Seitenleiste | indholdsoversigt |
| Vorschau | eksempel (eksempelfane) |
| Aktivität | Aktivitet |
| Projekt / Ordner | projekt / mappe |
| Vertrauen, vertrauen | tillid, stole på (betroet mappe) |
| Platte | disk (på disken) |
| Unterschiede | forskelle |
| Zusammenführen | merge (merge, merget, merge-konflikt) |
| Konflikt | konflikt |
| Treffer | resultat |
| Suche | Søgning |
| Ersetzen | Erstat |
| Sprachserver | sprogserver |
| Hinweis (Schweregrad) | Tip |
| Lesezeichen / Verlauf / Adresse | bogmærke / historik / adresse |
| Bytes | byte |
| Exportieren | Eksporter |
| Format | Format |
| Diktat | diktering |
| Agent | agent |
| Eigener Agent | Egen agent (in a sentence: appens egen agent) |
| Eigene Agenten | Brugerdefinerede agenter |
| Thread (AI-Thread, CLI-Thread) | tråd (AI-tråd, CLI-tråd) |
| Sitzung | session |
| Unterhaltung | samtale |
| Zug | runde |
| Antwort | svar |
| Rückfrage | spørgsmål |
| Freigabe, Freigabemodus | tilladelse, tilladelsestilstand |
| Umgehen | Omgå |
| Erlauben / Ablehnen / Immer erlauben … | Tillad / Afvis / Tillad altid… |
| Plan | plan |
| To-dos | Gøremål |
| Aufgabe | opgave |
| Auftrag | opgave |
| Werkzeug | værktøj |
| Skills | Skills |
| Befehl | kommando |
| Hook | hook |
| Sandbox | sandbox |
| Fremder Inhalt | Fremmed indhold |
| Sicherungspunkt | kontrolpunkt |
| Zurückspulen | spol tilbage (tilbagespoling) |
| Verdichten | komprimere (komprimering) |
| Kontext, Kontextfenster | kontekst, kontekstvindue |
| Denktiefe | tænkedybde |
| Modell / Anbieter / Endpunkt | model / udbyder / slutpunkt |
| Token, Tokens | token, tokens |
| API-Schlüssel | API-nøgle |
| Register | register |
| Adapter | adapter |
| Bereit | Klar |
| Prüfsumme | kontrolsum |
| Anmeldung, anmelden | login, log ind |
| Konto | konto |
| Abo | Abonnement |
| Repository | repository |
| Commit, committen | commit, committe |
| Secrets | secrets |
| vormerken | stage (staget, ikke staget) |
| Zurücknehmen | Unstage |
| nicht versioniert | ikke versionsstyret |
| Abschnitt (Git) | blok |
| Branch / Remote | branch / remote |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Übernehmen (Worktree) | overfør |
| Arbeitskopie | arbejdskopi |
| Lauf, Läufe | kørsel, kørsler |
| Host | vært |
| Job | job |
| Log | log |

### Greek (`el`)
- Address: formal plural (εσείς), imperatives such as "Επιλέξτε", "Εισαγάγετε", "Δοκιμάστε ξανά", as macOS Greek does;
  menu items and buttons are nouns of action ("Άνοιγμα", "Αποθήκευση ως…"), in sentence case
- Typography: «…» quotation marks (“…” inside them), ";" as the question mark, "Εύρεση…" with the ellipsis character and
  no space; "50%" without a space; units "ώ.", "λεπ.", "δευτ.", "χιλ.", "εκ."
- Git and CI words: kept as indeclinable neuter loanwords as Greek developers say them: commit, push, pull, fetch,
  rebase, cherry-pick, branch, remote, upstream, stage/unstage ("σε stage", "εκτός stage"), worktree, hook, job, token,
  sandbox, fast-forward; translated where Greek has an established word: αποθετήριο (repository), συγχώνευση (merge),
  διένεξη (conflict), τμήμα (hunk), αντίγραφο εργασίας (working tree), εκτέλεση (CI run), αρχείο καταγραφής (log); "AI"
  stays as "AI" ("καρτέλα AI", "νήμα AI")

| German key | Greek |
|---|---|
| Ablage | Αρχείο |
| Bearbeiten | Επεξεργασία |
| Darstellung | Προβολή |
| Fenster | Παράθυρο |
| Hilfe | Βοήθεια |
| Dienste | Υπηρεσίες |
| Über {} | Πληροφορίες για το {} |
| Einstellungen | Ρυθμίσεις |
| {} ausblenden / Andere ausblenden / Alle einblenden | Απόκρυψη {} / Απόκρυψη άλλων / Εμφάνιση όλων |
| {} beenden | Τερματισμός {} |
| Widerrufen / Wiederholen | Αναίρεση / Επανάληψη |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Αποκοπή / Αντιγραφή / Επικόλληση / Διαγραφή |
| Alles auswählen | Επιλογή όλων |
| Suchen … / Weitersuchen / Rückwärts suchen | Εύρεση… / Εύρεση επόμενου / Εύρεση προηγούμενου |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Ελαχιστοποίηση / Ζουμ / Μεταφορά όλων σε πρώτο πλάνο |
| Vollbildmodus | Είσοδος σε πλήρη οθόνη |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Απόκρυψη γραμμής εργαλείων / Εμφάνιση/απόκρυψη πλαϊνής στήλης |
| Sichern | Αποθήκευση |
| Öffnen / Schließen | Άνοιγμα / Κλείσιμο |
| Abbrechen / OK | Ακύρωση / OK |
| Im Finder zeigen | Εμφάνιση στο Finder |
| Papierkorb | Κάδος |
| Schlüsselbund | κλειδοθήκη |
| Zwischenablage | πρόχειρο |
| Tab | καρτέλα |
| AI-Tab | καρτέλα AI |
| Panel | πλαίσιο |
| Seitenleiste | πλαϊνή στήλη |
| Vorschau | προεπισκόπηση |
| Aktivität | Δραστηριότητα |
| Projekt / Ordner | έργο / φάκελος |
| Vertrauen, vertrauen | εμπιστοσύνη, εμπιστεύομαι |
| Platte | δίσκος |
| Unterschiede | διαφορές |
| Zusammenführen | συγχώνευση |
| Konflikt | διένεξη |
| Treffer | αποτέλεσμα |
| Suche | Αναζήτηση |
| Ersetzen | Αντικατάσταση |
| Sprachserver | διακομιστής γλώσσας |
| Hinweis (Schweregrad) | Υπόδειξη |
| Lesezeichen / Verlauf / Adresse | σελιδοδείκτης / ιστορικό / διεύθυνση |
| Bytes | byte |
| Exportieren | Εξαγωγή |
| Format | Μορφή |
| Diktat | υπαγόρευση |
| Agent | πράκτορας |
| Eigener Agent | ενσωματωμένος πράκτορας |
| Eigene Agenten | Προσαρμοσμένοι πράκτορες |
| Thread (AI-Thread, CLI-Thread) | νήμα (νήμα AI, νήμα CLI) |
| Sitzung | συνεδρία |
| Unterhaltung | συνομιλία |
| Zug | γύρος |
| Antwort | απάντηση |
| Rückfrage | ερώτηση |
| Freigabe, Freigabemodus | άδεια, λειτουργία αδειών |
| Umgehen | Παράκαμψη |
| Erlauben / Ablehnen / Immer erlauben … | Να επιτρέπεται / Άρνηση / Να επιτρέπεται πάντα… |
| Plan | σχέδιο |
| To-dos | Εκκρεμότητες |
| Aufgabe | εργασία |
| Auftrag | ανάθεση |
| Werkzeug | εργαλείο |
| Skills | Δεξιότητες |
| Befehl | εντολή |
| Hook | hook |
| Sandbox | sandbox |
| Fremder Inhalt | Εξωτερικό περιεχόμενο |
| Sicherungspunkt | σημείο ελέγχου |
| Zurückspulen | επιστροφή |
| Verdichten | συμπύκνωση |
| Kontext, Kontextfenster | συμφραζόμενα, παράθυρο συμφραζομένων |
| Denktiefe | βάθος σκέψης |
| Modell / Anbieter / Endpunkt | μοντέλο / πάροχος / τελικό σημείο |
| Token, Tokens | token (indeclinable) |
| API-Schlüssel | κλειδί API |
| Register | μητρώο |
| Adapter | προσαρμογέας |
| Bereit | Έτοιμο |
| Prüfsumme | άθροισμα ελέγχου |
| Anmeldung, anmelden | σύνδεση, συνδέομαι |
| Konto | λογαριασμός |
| Abo | Συνδρομή |
| Repository | αποθετήριο |
| Commit, committen | commit, κάνω commit |
| Secrets | μυστικά |
| vormerken | κάνω stage (σε stage / εκτός stage) |
| Zurücknehmen | Unstage |
| nicht versioniert | μη παρακολουθούμενο |
| Abschnitt (Git) | τμήμα |
| Branch / Remote | branch / remote |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Übernehmen (Worktree) | ενσωμάτωση |
| Arbeitskopie | αντίγραφο εργασίας |
| Lauf, Läufe | εκτέλεση, εκτελέσεις |
| Host | host |
| Job | job |
| Log | αρχείο καταγραφής |

### Spanish (`es`)
- Address: "tú", as macOS in Spanish (Spain) does: imperatives "Introduce", "Elige", "Abre"; possessives "tu", "tus"
- Typography: «comillas angulares» as splat uses them, “inglesas” only inside them; ellipsis "…" directly after the
  word, without a space, as in "Buscar…" and "Abrir carpeta…"; "50 %" with a space; menu items and buttons in sentence
  case, infinitive verbs ("Guardar como…", "Cerrar pestaña"); errors as "No se ha podido…"
- Git and CI words: kept as Spanish developers say them: commit ("hacer commit"), push, pull, fetch, rebase,
  cherry-pick, revert, fast-forward, upstream, hook, worktree, diff, job, log, token, endpoint, sandbox (masculine);
  translated where Spanish Git usage (Pro Git, git's own messages) has a word: rama, remoto, repositorio, fusionar,
  preparar/retirar (stage/unstage), fragmento (hunk), sin seguimiento (untracked), copia de trabajo, ejecución (CI run);
  "AI" is "IA"

| German key | Spanish |
|---|---|
| Ablage | Archivo |
| Bearbeiten | Edición |
| Darstellung | Visualización |
| Fenster | Ventana |
| Hilfe | Ayuda |
| Dienste | Servicios |
| Über {} | Acerca de {} |
| Einstellungen | Ajustes |
| {} ausblenden / Andere ausblenden / Alle einblenden | Ocultar {} / Ocultar otras apps / Mostrar todo |
| {} beenden | Salir de {} |
| Widerrufen / Wiederholen | Deshacer / Rehacer |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Cortar / Copiar / Pegar / Eliminar |
| Alles auswählen | Seleccionar todo |
| Suchen … / Weitersuchen / Rückwärts suchen | Buscar… / Buscar siguiente / Buscar anterior |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Minimizar / Zoom / Traer todo al frente |
| Vollbildmodus | Usar pantalla completa |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Ocultar barra de herramientas / Mostrar u ocultar barra lateral |
| Sichern | Guardar |
| Öffnen / Schließen | Abrir / Cerrar |
| Abbrechen / OK | Cancelar / Aceptar |
| Im Finder zeigen | Mostrar en el Finder |
| Papierkorb | papelera (Trasladar a la papelera) |
| Schlüsselbund | llavero |
| Zwischenablage | portapapeles |
| Tab | pestaña |
| AI-Tab | pestaña de IA |
| Panel | panel |
| Seitenleiste | barra lateral |
| Vorschau | vista previa |
| Aktivität | Actividad |
| Projekt / Ordner | proyecto / carpeta |
| Vertrauen, vertrauen | confianza, confiar en (carpeta de confianza) |
| Platte | disco (en el disco) |
| Unterschiede | diferencias |
| Zusammenführen | fusionar |
| Konflikt | conflicto |
| Treffer | resultado |
| Suche | Búsqueda |
| Ersetzen | Reemplazar |
| Sprachserver | servidor de lenguaje |
| Hinweis (Schweregrad) | Sugerencia |
| Lesezeichen / Verlauf / Adresse | marcador / historial / dirección |
| Bytes | bytes |
| Exportieren | Exportar |
| Format | Formato |
| Diktat | dictado |
| Agent | agente |
| Eigener Agent | agente propio |
| Eigene Agenten | Agentes personalizados |
| Thread (AI-Thread, CLI-Thread) | hilo (hilo de IA, hilo de CLI) |
| Sitzung | sesión |
| Unterhaltung | conversación |
| Zug | turno |
| Antwort | respuesta |
| Rückfrage | pregunta |
| Freigabe, Freigabemodus | permiso, modo de permisos |
| Umgehen | Omitir permisos |
| Erlauben / Ablehnen / Immer erlauben … | Permitir / Denegar / Permitir siempre… |
| Plan | plan |
| To-dos | Pendientes |
| Aufgabe | tarea |
| Auftrag | encargo |
| Werkzeug | herramienta |
| Skills | Skills |
| Befehl | comando |
| Hook | hook |
| Sandbox | sandbox (el sandbox) |
| Fremder Inhalt | Contenido externo |
| Sicherungspunkt | punto de control |
| Zurückspulen | rebobinar |
| Verdichten | compactar |
| Kontext, Kontextfenster | contexto, ventana de contexto |
| Denktiefe | razonamiento (bajo, medio, alto, muy alto, máximo) |
| Modell / Anbieter / Endpunkt | modelo / proveedor / endpoint |
| Token, Tokens | token, tokens |
| API-Schlüssel | clave de API |
| Register | registro |
| Adapter | adaptador |
| Bereit | Listo |
| Prüfsumme | suma de comprobación |
| Anmeldung, anmelden | inicio de sesión, iniciar sesión |
| Konto | cuenta |
| Abo | Suscripción |
| Repository | repositorio |
| Commit, committen | commit, hacer commit |
| Secrets | secretos |
| vormerken | preparar (zurücknehmen: retirar) |
| Zurücknehmen | Retirar |
| nicht versioniert | sin seguimiento |
| Abschnitt (Git) | fragmento |
| Branch / Remote | rama / remoto |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Übernehmen (Worktree) | incorporar |
| Arbeitskopie | copia de trabajo |
| Lauf, Läufe | ejecución, ejecuciones |
| Host | host |
| Job | job |
| Log | log |

### Finnish (`fi`)
- Address: informal singular "sinä", as Finnish macOS uses it ("Haluatko tallentaa…?", imperatives "Valitse", "Avaa");
  placeholders stay in the nominative, with Apple's colon form ("Tietoja: {}") or a head noun ("palveluun {}", "haaraan
  {}") where Finnish would inflect them
- Typography: ”…” (the same right double quotation mark on both sides) and ’ as apostrophe; menu items that open a
  dialog end in "…" without a space ("Avaa…", "Asetukset…"); a space before % ("50 %"), "t" for hours ("{} t")
- Git and CI words: commit (verb "commitoida", button "Commitoi"), Push, Pull, Fetch, Rebase, Revert, Cherry-pick,
  merge-commit, worktree and hook stay as loanwords with Finnish endings; branch → haara, remote → etärepositorio
  (remote branch → etähaara), repository → repositorio, hunk → lohko, stage → valmistella (staged → valmisteltu, unstage
  → peru valmistelu), untracked → seuraamaton, working tree → työkopio, CI run → ajo, job → työ, log → loki, token →
  token; "AI" stays as in the German key (AI-välilehti, AI-ketju)

| German key | Finnish |
|---|---|
| Ablage | Tiedosto |
| Bearbeiten | Muokkaa |
| Darstellung | Näytä |
| Fenster | Ikkuna |
| Hilfe | Ohje |
| Dienste | Palvelut |
| Über {} | Tietoja: {} |
| Einstellungen | Asetukset |
| {} ausblenden / Andere ausblenden / Alle einblenden | Kätke {} / Kätke muut / Näytä kaikki |
| {} beenden | Lopeta {} |
| Widerrufen / Wiederholen | Peru / Tee sittenkin |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Leikkaa / Kopioi / Sijoita / Poista |
| Alles auswählen | Valitse kaikki |
| Suchen … / Weitersuchen / Rückwärts suchen | Etsi… / Etsi seuraava / Etsi edellinen |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Pienennä / Zoomaa / Tuo kaikki eteen |
| Vollbildmodus | Siirry koko näytön tilaan |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Kätke työkalupalkki / Näytä tai kätke sivupalkki |
| Sichern | Tallenna |
| Öffnen / Schließen | Avaa / Sulje |
| Abbrechen / OK | Kumoa / OK |
| Im Finder zeigen | Näytä Finderissa |
| Papierkorb | Roskakori |
| Schlüsselbund | Avainnippu |
| Zwischenablage | Leikepöytä |
| Tab | välilehti |
| AI-Tab | AI-välilehti |
| Panel | paneeli |
| Seitenleiste | sivupalkki |
| Vorschau | esikatselu |
| Aktivität | Toiminta |
| Projekt / Ordner | projekti / kansio |
| Vertrauen, vertrauen | luottamus, luottaa (button "Luota") |
| Platte | levy ("levyllä") |
| Unterschiede | erot |
| Zusammenführen | yhdistäminen, yhdistää |
| Konflikt | ristiriita |
| Treffer | tulos (as splat: "Ei tuloksia") |
| Suche | Haku |
| Ersetzen | Korvaa |
| Sprachserver | kielipalvelin |
| Hinweis (Schweregrad) | Vihje |
| Lesezeichen / Verlauf / Adresse | kirjanmerkki / historia / osoite |
| Bytes | tavua |
| Exportieren | Vie |
| Format | Muoto |
| Diktat | sanelu |
| Agent | agentti |
| Eigener Agent | oma agentti |
| Eigene Agenten | Mukautetut agentit |
| Thread (AI-Thread, CLI-Thread) | ketju (AI-ketju, CLI-ketju) |
| Sitzung | istunto |
| Unterhaltung | keskustelu |
| Zug | vuoro |
| Antwort | vastaus |
| Rückfrage | kysymys |
| Freigabe, Freigabemodus | lupa, lupatila |
| Umgehen | Ohita |
| Erlauben / Ablehnen / Immer erlauben … | Salli / Hylkää / Salli aina… |
| Plan | suunnitelma |
| To-dos | Tehtävälista |
| Aufgabe | tehtävä |
| Auftrag | toimeksianto |
| Werkzeug | työkalu |
| Skills | Taidot |
| Befehl | komento |
| Hook | hook |
| Sandbox | hiekkalaatikko |
| Fremder Inhalt | Ulkopuolinen sisältö |
| Sicherungspunkt | tarkistuspiste |
| Zurückspulen | kelata taaksepäin, taaksepäin kelaus |
| Verdichten | tiivistää, tiivistäminen |
| Kontext, Kontextfenster | konteksti, konteksti-ikkuna |
| Denktiefe | ajattelun syvyys |
| Modell / Anbieter / Endpunkt | malli / palveluntarjoaja / päätepiste |
| Token, Tokens | token, tokenit ("{} tokenia") |
| API-Schlüssel | API-avain |
| Register | rekisteri |
| Adapter | sovitin |
| Bereit | Valmis |
| Prüfsumme | tarkistussumma |
| Anmeldung, anmelden | kirjautuminen, kirjautua sisään |
| Konto | tili |
| Abo | Tilaus |
| Repository | repositorio |
| Commit, committen | commit, commitoida |
| Secrets | salaisuudet |
| vormerken | valmistella |
| Zurücknehmen | Peru valmistelu |
| nicht versioniert | seuraamaton |
| Abschnitt (Git) | lohko |
| Branch / Remote | haara / etärepositorio |
| Push / Pull / Fetch / Rebase | Push / Pull / Fetch / Rebase |
| Worktree | worktree |
| Übernehmen (Worktree) | ota käyttöön |
| Arbeitskopie | työkopio |
| Lauf, Läufe | ajo, ajot |
| Host | palvelin |
| Job | työ |
| Log | loki |

### French (`fr`)
- Address: "vous", with "vous" imperatives ("Saisissez…", "Ouvrez…"), as macOS does
- Typography: « guillemets » with a no-break space inside, no-break space before : ; ? ! and %, apostrophe ’; ellipsis
  attached without a space, as in macOS menus ("Réglages…", "Enregistrer sous…"); sentence case everywhere
- Git and CI words: kept as developers say them: commit (verb "commiter"), push, pull, fetch, rebase, cherry-pick,
  revert, hook, worktree, job, diff, upstream; translated where French Git usage is settled: dépôt, branche, dépôt
  distant (remote), indexer/désindexer (stage/unstage), bloc (hunk), fusionner/fusion (merge), copie de travail, non
  suivi (untracked); CI: exécution (run), journal (log), jeton (token). "AI" is "IA" (onglet IA, fil IA)

| German key | French |
|---|---|
| Ablage | Fichier |
| Bearbeiten | Édition |
| Darstellung | Présentation |
| Fenster | Fenêtre |
| Hilfe | Aide |
| Dienste | Services |
| Über {} | À propos de {} |
| Einstellungen | Réglages (menu item: Réglages…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | Masquer {} / Masquer les autres / Tout afficher |
| {} beenden | Quitter {} |
| Widerrufen / Wiederholen | Annuler / Rétablir |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Couper / Copier / Coller / Supprimer |
| Alles auswählen | Tout sélectionner |
| Suchen … / Weitersuchen / Rückwärts suchen | Rechercher… / Rechercher le suivant / Rechercher le précédent |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Minimiser / Réduire/agrandir / Tout ramener au premier plan |
| Vollbildmodus | Activer le mode plein écran |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Masquer la barre d’outils / Afficher/masquer la barre latérale |
| Sichern | Enregistrer |
| Öffnen / Schließen | Ouvrir / Fermer |
| Abbrechen / OK | Annuler / OK |
| Im Finder zeigen | Afficher dans le Finder |
| Papierkorb | corbeille (Placer dans la corbeille) |
| Schlüsselbund | trousseau |
| Zwischenablage | presse-papiers |
| Tab | onglet |
| AI-Tab | onglet IA |
| Panel | panneau |
| Seitenleiste | barre latérale |
| Vorschau | aperçu (onglet d’aperçu) |
| Aktivität | Activité |
| Projekt / Ordner | projet / dossier |
| Vertrauen, vertrauen | confiance, faire confiance (dossier de confiance; not trusted: non approuvé) |
| Platte | disque (sur le disque) |
| Unterschiede | différences |
| Zusammenführen | fusionner, fusion |
| Konflikt | conflit |
| Treffer | résultat |
| Suche | Recherche |
| Ersetzen | Remplacer |
| Sprachserver | serveur de langage |
| Hinweis (Schweregrad) | Suggestion |
| Lesezeichen / Verlauf / Adresse | signet / historique / adresse |
| Bytes | octets |
| Exportieren | Exporter |
| Format | Format |
| Diktat | dictée |
| Agent | agent |
| Eigener Agent | agent intégré |
| Eigene Agenten | Agents personnalisés |
| Thread (AI-Thread, CLI-Thread) | fil (fil IA, fil CLI) |
| Sitzung | session |
| Unterhaltung | conversation |
| Zug | tour |
| Antwort | réponse |
| Rückfrage | question |
| Freigabe, Freigabemodus | autorisation, mode d’autorisation |
| Umgehen | Contourner |
| Erlauben / Ablehnen / Immer erlauben … | Autoriser / Refuser / Toujours autoriser… |
| Plan | plan |
| To-dos | À faire |
| Aufgabe | tâche |
| Auftrag | mission |
| Werkzeug | outil |
| Skills | Skills |
| Befehl | commande |
| Hook | hook |
| Sandbox | sandbox (la sandbox) |
| Fremder Inhalt | Contenu externe |
| Sicherungspunkt | point de contrôle |
| Zurückspulen | revenir en arrière, retour en arrière |
| Verdichten | compacter, compactage |
| Kontext, Kontextfenster | contexte, fenêtre de contexte |
| Denktiefe | niveau de réflexion |
| Modell / Anbieter / Endpunkt | modèle / fournisseur / point de terminaison |
| Token, Tokens | jeton, jetons |
| API-Schlüssel | clé d’API |
| Register | registre |
| Adapter | adaptateur |
| Bereit | Prêt |
| Prüfsumme | somme de contrôle |
| Anmeldung, anmelden | connexion, se connecter |
| Konto | compte |
| Abo | Abonnement |
| Repository | dépôt |
| Commit, committen | commit, commiter |
| Secrets | secrets |
| vormerken | indexer (zurücknehmen: désindexer) |
| Zurücknehmen | Désindexer |
| nicht versioniert | non suivi |
| Abschnitt (Git) | bloc |
| Branch / Remote | branche / dépôt distant |
| Push / Pull / Fetch / Rebase | Push / Pull / Fetch / Rebase |
| Worktree | worktree |
| Übernehmen (Worktree) | intégrer |
| Arbeitskopie | copie de travail |
| Lauf, Läufe | exécution, exécutions |
| Host | hôte |
| Job | job |
| Log | journal |

### Hebrew (`he`)
- Address: neutral, as macOS Hebrew: commands and buttons as verbal nouns ("שמירה", "סגירת הכרטיסיה"), confirmation
  questions in the infinitive ("לשמור שינויים ב-״{}״?"), instructions impersonal ("יש לבחור…", "ניתן…", "באפשרותך…"); no
  gendered imperatives or pronouns; agents and apps take masculine verbs, as macOS does for apps
- Typography: gershayim quotes ״…״, geresh in abbreviations (דק׳, שע׳); the ellipsis directly after the word, no space
  ("שמירה…"); a hyphen joins a Hebrew prefix to a Latin word or placeholder ("ב-Finder", "ה-Commit", "ב-{}"); as in
  macOS, an invisible right-to-left mark (U+200F) opens a text that starts with a Latin word or a name placeholder, and
  a left-to-right mark (U+200E) keeps a command or shortcut such as "/model" or "⌘Esc" in its own order
- Git and CI words: Git's own names stay in Latin script, as Hebrew developers write them: Commit, Branch, Remote, Push,
  Pull, Fetch, Rebase, Cherry-Pick, Revert, Upstream, Fast-Forward, Worktree, Hook, Job; general ideas are Hebrew: מאגר
  (repository), מיזוג (merge), התנגשות (conflict), קטע (hunk), סימון ל-Commit (stage), עותק העבודה (working tree), הרצה
  (CI run), יומן (log); "AI" stays "AI"

| German key | Hebrew |
|---|---|
| Ablage | קובץ |
| Bearbeiten | עריכה |
| Darstellung | תצוגה |
| Fenster | חלון |
| Hilfe | עזרה |
| Dienste | שירותים |
| Über {} | מידע על {} |
| Einstellungen | הגדרות |
| {} ausblenden / Andere ausblenden / Alle einblenden | הסתרת {} / הסתרת יישומים אחרים / הצגת הכול |
| {} beenden | סיום {} |
| Widerrufen / Wiederholen | ביטול הפעולה האחרונה / שחזור הפעולה האחרונה |
| Ausschneiden / Kopieren / Einsetzen / Löschen | גזירה / העתקה / הדבקה / מחיקה |
| Alles auswählen | בחירת הכול |
| Suchen … / Weitersuchen / Rückwärts suchen | חיפוש… / מציאת הבא / מציאת הקודם |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | מזעור / זום / הבאת כל החלונות קדימה |
| Vollbildmodus | מעבר למסך מלא |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | הסתרת סרגל הכלים / הצגה או הסתרה של סרגל הצד |
| Sichern | שמירה |
| Öffnen / Schließen | פתיחה / סגירה |
| Abbrechen / OK | ביטול / אישור |
| Im Finder zeigen | הצגה ב-Finder |
| Papierkorb | פח האשפה ("In den Papierkorb legen": העברה לפח) |
| Schlüsselbund | צרור המפתחות |
| Zwischenablage | לוח העריכה |
| Tab | כרטיסיה |
| AI-Tab | כרטיסיית AI |
| Panel | חלונית |
| Seitenleiste | סרגל צד |
| Vorschau | תצוגה מקדימה |
| Aktivität | פעילות |
| Projekt / Ordner | פרויקט / תיקיה |
| Vertrauen, vertrauen | אמון, לתת אמון (תיקיה מהימנה) |
| Platte | דיסק |
| Unterschiede | הבדלים |
| Zusammenführen | מיזוג |
| Konflikt | התנגשות |
| Treffer | תוצאה |
| Suche | חיפוש |
| Ersetzen | החלפה |
| Sprachserver | שרת שפה |
| Hinweis (Schweregrad) | רמז |
| Lesezeichen / Verlauf / Adresse | סימניה / היסטוריה / כתובת |
| Bytes | בתים |
| Exportieren | ייצוא |
| Format | עיצוב |
| Diktat | הכתבה |
| Agent | סוכן |
| Eigener Agent | סוכן מובנה |
| Eigene Agenten | סוכנים מותאמים אישית |
| Thread (AI-Thread, CLI-Thread) | שרשור (שרשור AI, שרשור CLI) |
| Sitzung | סשן |
| Unterhaltung | שיחה |
| Zug | סבב |
| Antwort | תשובה |
| Rückfrage | שאלה |
| Freigabe, Freigabemodus | הרשאה, מצב הרשאות |
| Umgehen | עקיפה |
| Erlauben / Ablehnen / Immer erlauben … | אישור / סירוב / הרשאה קבועה… |
| Plan | תוכנית |
| To-dos | משימות לביצוע |
| Aufgabe | משימה |
| Auftrag | משימה |
| Werkzeug | כלי |
| Skills | מיומנויות |
| Befehl | פקודה |
| Hook | Hook |
| Sandbox | ארגז חול |
| Fremder Inhalt | תוכן חיצוני |
| Sicherungspunkt | נקודת שחזור |
| Zurückspulen | חזרה לאחור |
| Verdichten | דחיסה |
| Kontext, Kontextfenster | הקשר, חלון ההקשר |
| Denktiefe | עומק חשיבה |
| Modell / Anbieter / Endpunkt | מודל / ספק / נקודת קצה |
| Token, Tokens | טוקן, טוקנים |
| API-Schlüssel | מפתח API |
| Register | מרשם |
| Adapter | מתאם |
| Bereit | מוכן |
| Prüfsumme | סכום ביקורת |
| Anmeldung, anmelden | התחברות, להתחבר |
| Konto | חשבון |
| Abo | מינוי |
| Repository | מאגר |
| Commit, committen | Commit, ביצוע Commit |
| Secrets | סודות |
| vormerken | סימון ל-Commit (מסומן, ביטול הסימון) |
| Zurücknehmen | ביטול הסימון |
| nicht versioniert | לא במעקב |
| Abschnitt (Git) | קטע |
| Branch / Remote | Branch / Remote |
| Push / Pull / Fetch / Rebase | Push / Pull / Fetch / Rebase |
| Worktree | Worktree |
| Übernehmen (Worktree) | שילוב |
| Arbeitskopie | עותק העבודה |
| Lauf, Läufe | הרצה, הרצות |
| Host | מארח |
| Job | Job |
| Log | יומן |

### Hindi (`hi`)
- Address: formal "आप", with polite "-ें/-एँ" imperatives ("खोलें", "दर्ज करें", "दिखाएँ"), as macOS does
- Typography: curly quotes “…”, sentences end with the danda "।", a space before a colon in labels ("नाम :"), as splat
  does; ellipsis attached without a space, as in macOS menus ("सेटिंग्ज़…", "ढूँढें…"); Western digits, nukta in
  loanwords as macOS writes them (फ़ाइल, ज़ूम)
- Git and CI words: written in Devanagari as Hindi-speaking developers say them: कमिट (कमिट करें), ब्रांच, रिमोट, पुश,
  पुल, फ़ेच, रीबेस, मर्ज, चेरी-पिक, रिवर्ट, हुक, वर्कट्री, अपस्ट्रीम, स्टेज/अनस्टेज करें, हंक, अनट्रैक्ड, रिपॉज़िटरी;
  CI: रन, जॉब, लॉग, टोकन; commands, flags and product names stay in Latin script. "AI" stays "AI" (AI टैब, AI थ्रेड)

| German key | Hindi |
|---|---|
| Ablage | फ़ाइल |
| Bearbeiten | संपादन |
| Darstellung | दृश्य |
| Fenster | विंडो |
| Hilfe | सहायता |
| Dienste | सेवाएँ |
| Über {} | {} का परिचय |
| Einstellungen | सेटिंग्ज़ (menu item: सेटिंग…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | {} छिपाएँ / अन्य छिपाएँ / सभी दिखाएँ |
| {} beenden | {} बंद करें |
| Widerrufen / Wiederholen | पहले जैसा करें / फिर करें |
| Ausschneiden / Kopieren / Einsetzen / Löschen | कट / कॉपी / पेस्ट / डिलीट करें |
| Alles auswählen | सभी चुनें |
| Suchen … / Weitersuchen / Rückwärts suchen | ढूँढें… / अगला ढूँढें / पिछला ढूँढें |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | मिनिमाइज़ करें / ज़ूम करें / सभी आगे लाएँ |
| Vollbildmodus | फ़ुल स्‍क्रीन में प्रवेश करें |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | टूलबार छिपाएँ / साइडबार दिखाएँ या छिपाएँ |
| Sichern | सहेजें |
| Öffnen / Schließen | खोलें / बंद करें |
| Abbrechen / OK | रद्द करें / ठीक |
| Im Finder zeigen | Finder में दिखाएँ |
| Papierkorb | ट्रैश |
| Schlüsselbund | कीचेन |
| Zwischenablage | क्लिपबोर्ड |
| Tab | टैब |
| AI-Tab | AI टैब |
| Panel | पैनल |
| Seitenleiste | साइडबार |
| Vorschau | प्रीव्यू |
| Aktivität | ऐक्टिविटी |
| Projekt / Ordner | प्रोजेक्ट / फ़ोल्डर |
| Vertrauen, vertrauen | भरोसा, भरोसा करना (भरोसेमंद फ़ोल्डर) |
| Platte | डिस्क (डिस्क पर) |
| Unterschiede | अंतर |
| Zusammenführen | मर्ज करना |
| Konflikt | कॉन्फ़्लिक्ट |
| Treffer | परिणाम |
| Suche | खोज |
| Ersetzen | बदलें |
| Sprachserver | लैंग्वेज सर्वर |
| Hinweis (Schweregrad) | संकेत |
| Lesezeichen / Verlauf / Adresse | बुकमार्क / इतिहास / पता |
| Bytes | बाइट |
| Exportieren | एक्सपोर्ट करें |
| Format | फ़ॉर्मैट |
| Diktat | डिक्टेशन |
| Agent | एजेंट |
| Eigener Agent | बिल्ट-इन एजेंट |
| Eigene Agenten | कस्टम एजेंट |
| Thread (AI-Thread, CLI-Thread) | थ्रेड (AI थ्रेड, CLI थ्रेड) |
| Sitzung | सेशन |
| Unterhaltung | बातचीत |
| Zug | टर्न |
| Antwort | जवाब |
| Rückfrage | सवाल |
| Freigabe, Freigabemodus | अनुमति, अनुमति मोड |
| Umgehen | बायपास |
| Erlauben / Ablehnen / Immer erlauben … | अनुमति दें / अस्वीकार करें / हमेशा अनुमति दें… |
| Plan | योजना |
| To-dos | टू-डू |
| Aufgabe | कार्य |
| Auftrag | कार्य |
| Werkzeug | टूल |
| Skills | स्किल |
| Befehl | कमांड |
| Hook | हुक |
| Sandbox | सैंडबॉक्स |
| Fremder Inhalt | बाहरी कॉन्टेंट |
| Sicherungspunkt | चेकपॉइंट |
| Zurückspulen | रिवाइंड करना |
| Verdichten | कॉम्पैक्ट करना |
| Kontext, Kontextfenster | कॉन्टेक्स्ट, कॉन्टेक्स्ट विंडो |
| Denktiefe | सोच का स्तर |
| Modell / Anbieter / Endpunkt | मॉडल / प्रदाता / एंडपॉइंट |
| Token, Tokens | टोकन |
| API-Schlüssel | API कुंजी |
| Register | रजिस्ट्री |
| Adapter | एडैप्टर |
| Bereit | तैयार |
| Prüfsumme | चेकसम |
| Anmeldung, anmelden | साइन इन, साइन इन करें |
| Konto | खाता |
| Abo | सब्सक्रिप्शन |
| Repository | रिपॉज़िटरी |
| Commit, committen | कमिट, कमिट करें |
| Secrets | सीक्रेट |
| vormerken | स्टेज करें (zurücknehmen: अनस्टेज करें) |
| Zurücknehmen | अनस्टेज करें |
| nicht versioniert | अनट्रैक्ड |
| Abschnitt (Git) | हंक |
| Branch / Remote | ब्रांच / रिमोट |
| Push / Pull / Fetch / Rebase | पुश / पुल / फ़ेच / रीबेस |
| Worktree | वर्कट्री |
| Übernehmen (Worktree) | शामिल करना |
| Arbeitskopie | वर्किंग ट्री |
| Lauf, Läufe | रन |
| Host | होस्ट |
| Job | जॉब |
| Log | लॉग |

### Croatian (`hr`)
- Address: formal "vi" in sentences, lowercase as macOS writes it ("Želite li…", "Odaberite…", "vaš"); menu items and
  buttons in the singular imperative as macOS has them ("Spremi", "Otvori", "Odustani")
- Typography: “…” (U+201C/U+201D, as macOS Croatian quotes), "Postavke…" with no space before the ellipsis; counts that
  would need plural forms are written "Datoteke: {}" as splat does
- Git and CI words: commit, push, pull, fetch, rebase, cherry-pick, worktree, hook, sandbox and token stay loanwords and
  are declined (commita, commitovi); branch is "grana", remote "udaljeni repozitorij", merge "spajanje" (also as the
  button and the operation name), stage "pripremiti (za commit)", hunk "blok", a CI run "pokretanje", a job "posao", a
  log "zapisnik"

| German key | Croatian |
|---|---|
| Ablage | Datoteka |
| Bearbeiten | Uredi |
| Darstellung | Prikaz |
| Fenster | Prozor |
| Hilfe | Pomoć |
| Dienste | Usluge |
| Über {} | Više o aplikaciji {} |
| Einstellungen | Postavke |
| {} ausblenden / Andere ausblenden / Alle einblenden | Sakrij {} / Sakrij ostalo / Prikaži sve |
| {} beenden | Zatvori {} |
| Widerrufen / Wiederholen | Poništi / Ponovi |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Izreži / Kopiraj / Zalijepi / Obriši |
| Alles auswählen | Odaberi sve |
| Suchen … / Weitersuchen / Rückwärts suchen | Pronađi… / Pronađi sljedeće / Pronađi prethodno |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Smanji / Zumiraj / Postavi sve naprijed |
| Vollbildmodus | Prikaži preko cijelog zaslona |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Sakrij alatnu traku / Prikaži/sakrij rubni stupac |
| Sichern | Spremi |
| Öffnen / Schließen | Otvori / Zatvori |
| Abbrechen / OK | Odustani / U redu |
| Im Finder zeigen | Prikaži u Finderu |
| Papierkorb | Smeće |
| Schlüsselbund | privjesak ključeva |
| Zwischenablage | memorijski pretinac |
| Tab | kartica |
| AI-Tab | AI kartica |
| Panel | okno |
| Seitenleiste | rubni stupac |
| Vorschau | pregled |
| Aktivität | Aktivnost |
| Projekt / Ordner | projekt / mapa |
| Vertrauen, vertrauen | povjerenje, vjerovati (trusted: pouzdan) |
| Platte | disk |
| Unterschiede | razlike |
| Zusammenführen | spajanje |
| Konflikt | konflikt |
| Treffer | rezultat |
| Suche | Pretraga |
| Ersetzen | Zamijeni |
| Sprachserver | jezični poslužitelj |
| Hinweis (Schweregrad) | Savjet |
| Lesezeichen / Verlauf / Adresse | knjižna oznaka / povijest / adresa |
| Bytes | B |
| Exportieren | Eksportiraj |
| Format | Formatiraj |
| Diktat | diktat |
| Agent | agent |
| Eigener Agent | vlastiti agent |
| Eigene Agenten | Prilagođeni agenti |
| Thread (AI-Thread, CLI-Thread) | nit (AI nit, CLI nit) |
| Sitzung | sesija |
| Unterhaltung | razgovor |
| Zug | krug |
| Antwort | odgovor |
| Rückfrage | pitanje |
| Freigabe, Freigabemodus | dozvola, način dozvola |
| Umgehen | Zaobilaženje |
| Erlauben / Ablehnen / Immer erlauben … | Dozvoli / Odbij / Uvijek dozvoli… |
| Plan | plan |
| To-dos | Obaveze |
| Aufgabe | zadatak |
| Auftrag | zadatak |
| Werkzeug | alat |
| Skills | Vještine |
| Befehl | naredba |
| Hook | hook |
| Sandbox | sandbox |
| Fremder Inhalt | Vanjski sadržaj |
| Sicherungspunkt | kontrolna točka |
| Zurückspulen | premotavanje, Premotaj |
| Verdichten | sažimanje, Sažmi |
| Kontext, Kontextfenster | kontekst, kontekstni prozor |
| Denktiefe | dubina razmišljanja |
| Modell / Anbieter / Endpunkt | model / pružatelj / krajnja točka |
| Token, Tokens | token, tokeni |
| API-Schlüssel | API ključ |
| Register | registar |
| Adapter | adapter |
| Bereit | Spremno |
| Prüfsumme | kontrolni zbroj |
| Anmeldung, anmelden | prijava, prijaviti se |
| Konto | račun |
| Abo | Pretplata |
| Repository | repozitorij |
| Commit, committen | commit, izraditi commit |
| Secrets | tajni podaci |
| vormerken | pripremiti (za commit) |
| Zurücknehmen | Ukloni iz pripreme |
| nicht versioniert | nepraćeno |
| Abschnitt (Git) | blok |
| Branch / Remote | grana / udaljeni repozitorij |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Übernehmen (Worktree) | preuzeti |
| Arbeitskopie | radna kopija |
| Lauf, Läufe | pokretanje, pokretanja |
| Host | host |
| Job | posao |
| Log | zapisnik |

### Hungarian (`hu`)
- Address: formal, as Hungarian macOS: the polite third-person (Ön) verb forms with the pronoun left out ("Biztosan
  törli?", "Csak akkor nyissa meg a linket, ha megbízik…"); menu items and buttons as action nouns ("Megnyitás",
  "Mentés", "Kilépés")
- Typography: „…” quotation marks; ellipsis "…" directly after the word, no space ("Beállítások…", "Keresés…"); sentence
  case; before a `{}` the article is written "a(z)", as macOS does ("A(z) {} névjegye")
- Git and CI words: Git's operations keep their English names (commit/commitol, push, pull, fetch, rebase, worktree,
  hook, job, token); translated where Hungarian Git tools (git-gui, Visual Studio) have a settled word: tároló
  (repository), ág (branch), távoli tároló (remote), összefésülés (merge), előkészítés (stage), blokk (hunk),
  munkakönyvtár (working tree), futás (CI run), napló (log), ütközés (conflict); "AI" stays AI, as Hungarian macOS
  writes it

| German key | Hungarian |
|---|---|
| Ablage | Fájl |
| Bearbeiten | Szerkesztés |
| Darstellung | Nézet |
| Fenster | Ablak |
| Hilfe | Súgó |
| Dienste | Szolgáltatások |
| Über {} | A(z) {} névjegye |
| Einstellungen | Beállítások (menu item: Beállítások…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | A(z) {} elrejtése / Többi elrejtése / Összes megjelenítése |
| {} beenden | Kilépés a(z) {} appból |
| Widerrufen / Wiederholen | Visszavonás / Ismétlés |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Kivágás / Másolás / Beillesztés / Törlés |
| Alles auswählen | Összes kijelölése |
| Suchen … / Weitersuchen / Rückwärts suchen | Keresés… / Következő keresése / Előző keresése |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Kis méret / Nagyítás / Összes előtérbe hozása |
| Vollbildmodus | Váltás teljes képernyőre |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Eszközsor elrejtése / Oldalsáv megjelenítése/elrejtése |
| Sichern | Mentés |
| Öffnen / Schließen | Megnyitás / Bezárás |
| Abbrechen / OK | Mégsem / OK |
| Im Finder zeigen | Megjelenítés a Finderben |
| Papierkorb | Kuka (Áthelyezés a kukába) |
| Schlüsselbund | kulcskarika |
| Zwischenablage | vágólap |
| Tab | lap |
| AI-Tab | AI-lap |
| Panel | panel |
| Seitenleiste | oldalsáv |
| Vorschau | előnézet |
| Aktivität | Tevékenység |
| Projekt / Ordner | projekt / mappa |
| Vertrauen, vertrauen | bizalom, megbízik (megbízható mappa) |
| Platte | lemez (a lemezen) |
| Unterschiede | különbségek |
| Zusammenführen | összefésülés |
| Konflikt | ütközés |
| Treffer | találat |
| Suche | Keresés |
| Ersetzen | Csere |
| Sprachserver | nyelvi szerver |
| Hinweis (Schweregrad) | Tipp |
| Lesezeichen / Verlauf / Adresse | könyvjelző / előzmények / cím |
| Bytes | bájt |
| Exportieren | Exportálás |
| Format | Formátum |
| Diktat | diktálás |
| Agent | ügynök |
| Eigener Agent | beépített ügynök |
| Eigene Agenten | Saját ügynökök |
| Thread (AI-Thread, CLI-Thread) | szál (AI-szál, CLI-szál) |
| Sitzung | munkamenet |
| Unterhaltung | beszélgetés |
| Zug | kör |
| Antwort | válasz |
| Rückfrage | kérdés |
| Freigabe, Freigabemodus | engedély, engedélymód |
| Umgehen | Megkerülés |
| Erlauben / Ablehnen / Immer erlauben … | Engedélyezés / Elutasítás / Engedélyezés mindig… |
| Plan | terv |
| To-dos | Teendők |
| Aufgabe | feladat |
| Auftrag | feladat |
| Werkzeug | eszköz |
| Skills | Skillek |
| Befehl | parancs |
| Hook | hook |
| Sandbox | homokozó |
| Fremder Inhalt | Külső tartalom |
| Sicherungspunkt | visszaállítási pont |
| Zurückspulen | visszatekerés |
| Verdichten | tömörítés |
| Kontext, Kontextfenster | kontextus, kontextusablak |
| Denktiefe | gondolkodási mélység |
| Modell / Anbieter / Endpunkt | modell / szolgáltató / végpont |
| Token, Tokens | token, tokenek |
| API-Schlüssel | API-kulcs |
| Register | jegyzék |
| Adapter | adapter |
| Bereit | Kész |
| Prüfsumme | ellenőrzőösszeg |
| Anmeldung, anmelden | bejelentkezés, bejelentkezik |
| Konto | fiók |
| Abo | Előfizetés |
| Repository | tároló |
| Commit, committen | commit, commitol |
| Secrets | titkos adatok |
| vormerken | előkészít (unstage: előkészítés visszavonása) |
| Zurücknehmen | Előkészítés visszavonása |
| nicht versioniert | nem követett |
| Abschnitt (Git) | blokk |
| Branch / Remote | ág / távoli tároló |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Übernehmen (Worktree) | átvétel |
| Arbeitskopie | munkakönyvtár |
| Lauf, Läufe | futás, futások |
| Host | hoszt |
| Job | job |
| Log | napló |

### Indonesian (`id`)
- Address: formal "Anda", capitalized, as Indonesian macOS ("Apakah Anda ingin…", "Buka tautan hanya jika Anda
  memercayai sumbernya"); menu items and buttons as imperative verbs ("Buka", "Simpan", "Tampilkan")
- Typography: “…” quotation marks; ellipsis "…" directly after the word, no space ("Pengaturan…", "Temukan…"); menu
  items, buttons and titles in Title Case, as Indonesian macOS ("Tampilkan Semua", "Pilih Semua"), running text in
  sentence case
- Git and CI words: kept as Indonesian developers say them: commit (melakukan commit), branch, remote, push, pull,
  fetch, rebase, stage/unstage, hunk, worktree, hook, job, log, run (CI), token, endpoint, sandbox, checksum; Indonesian
  where the word is settled: repositori, salinan kerja (working tree), gabungkan (merge), konflik; "AI" stays AI

| German key | Indonesian |
|---|---|
| Ablage | File |
| Bearbeiten | Edit |
| Darstellung | Lihat |
| Fenster | Jendela |
| Hilfe | Bantuan |
| Dienste | Layanan |
| Über {} | Mengenai {} |
| Einstellungen | Pengaturan (menu item: Pengaturan…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | Sembunyikan {} / Sembunyikan Lainnya / Tampilkan Semua |
| {} beenden | Tutup {} |
| Widerrufen / Wiederholen | Urung / Ulang |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Potong / Salin / Tempel / Hapus |
| Alles auswählen | Pilih Semua |
| Suchen … / Weitersuchen / Rückwärts suchen | Temukan… / Temukan Berikutnya / Temukan Sebelumnya |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Minikan / Zoom / Pindahkan Semua ke Depan |
| Vollbildmodus | Masuk ke Layar Penuh |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Sembunyikan Bar Alat / Ubah Bar Samping |
| Sichern | Simpan |
| Öffnen / Schließen | Buka / Tutup |
| Abbrechen / OK | Batalkan / OKE |
| Im Finder zeigen | Tampilkan di Finder |
| Papierkorb | Tong Sampah (Pindahkan ke Tong Sampah) |
| Schlüsselbund | Rantai Kunci |
| Zwischenablage | papan klip |
| Tab | tab |
| AI-Tab | tab AI |
| Panel | panel |
| Seitenleiste | bar samping |
| Vorschau | pratinjau |
| Aktivität | Aktivitas |
| Projekt / Ordner | proyek / folder |
| Vertrauen, vertrauen | kepercayaan, memercayai (folder tepercaya; button: Percayai) |
| Platte | disk (di disk) |
| Unterschiede | perbedaan |
| Zusammenführen | gabungkan (penggabungan) |
| Konflikt | konflik |
| Treffer | hasil |
| Suche | Pencarian |
| Ersetzen | Ganti |
| Sprachserver | server bahasa |
| Hinweis (Schweregrad) | Petunjuk |
| Lesezeichen / Verlauf / Adresse | penanda / riwayat / alamat |
| Bytes | byte |
| Exportieren | Ekspor |
| Format | Format |
| Diktat | dikte |
| Agent | agen |
| Eigener Agent | agen bawaan |
| Eigene Agenten | Agen kustom |
| Thread (AI-Thread, CLI-Thread) | thread (thread AI, thread CLI) |
| Sitzung | sesi |
| Unterhaltung | percakapan |
| Zug | giliran |
| Antwort | jawaban |
| Rückfrage | pertanyaan |
| Freigabe, Freigabemodus | izin, mode izin |
| Umgehen | Lewati |
| Erlauben / Ablehnen / Immer erlauben … | Izinkan / Tolak / Selalu Izinkan… |
| Plan | rencana |
| To-dos | Daftar Tugas |
| Aufgabe | tugas |
| Auftrag | tugas |
| Werkzeug | alat |
| Skills | Skill |
| Befehl | perintah |
| Hook | hook |
| Sandbox | sandbox |
| Fremder Inhalt | Konten Eksternal |
| Sicherungspunkt | titik pemulihan |
| Zurückspulen | putar balik |
| Verdichten | padatkan (pemadatan) |
| Kontext, Kontextfenster | konteks, jendela konteks |
| Denktiefe | kedalaman berpikir |
| Modell / Anbieter / Endpunkt | model / penyedia / endpoint |
| Token, Tokens | token |
| API-Schlüssel | kunci API |
| Register | registri |
| Adapter | adaptor |
| Bereit | Siap |
| Prüfsumme | checksum |
| Anmeldung, anmelden | masuk (Masuk ke {}), proses masuk |
| Konto | akun |
| Abo | Langganan |
| Repository | repositori |
| Commit, committen | commit, melakukan commit |
| Secrets | rahasia |
| vormerken | stage (batalkan stage) |
| Zurücknehmen | Batalkan Stage |
| nicht versioniert | tidak dilacak |
| Abschnitt (Git) | hunk |
| Branch / Remote | branch / remote |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Übernehmen (Worktree) | ambil |
| Arbeitskopie | salinan kerja |
| Lauf, Läufe | run |
| Host | host |
| Job | job |
| Log | log |

### Italian (`it`)
- Address: "tu", as Italian macOS ("Vuoi riprovare?", "Apri il link solo se ti fidi della fonte"); menu items and
  buttons as imperatives ("Apri", "Salva", "Mostra")
- Typography: “…” quotation marks, as Italian macOS writes them (not the «…» splat uses); typographic apostrophe ’
  ("un’altra", "dall’ultimo"); ellipsis "…" directly after the word, no space ("Impostazioni…", "Trova…"); sentence case
- Git and CI words: kept as Italian developers say them: repository (il repository), commit (fare il commit), branch,
  remote, push, pull, fetch, rebase, stage (mettere in stage / togliere dallo stage), worktree, hook, job, log, token,
  endpoint, sandbox, checksum, task; Italian where the word is settled: unire (merge), conflitto, blocco (hunk), copia
  di lavoro (working tree), esecuzione (CI run); "AI" stays AI ("pannello AI")

| German key | Italian |
|---|---|
| Ablage | File |
| Bearbeiten | Modifica |
| Darstellung | Vista |
| Fenster | Finestra |
| Hilfe | Aiuto |
| Dienste | Servizi |
| Über {} | Informazioni su {} |
| Einstellungen | Impostazioni (menu item: Impostazioni…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | Nascondi {} / Nascondi altre / Mostra tutte |
| {} beenden | Esci da {} |
| Widerrufen / Wiederholen | Annulla / Ripristina |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Taglia / Copia / Incolla / Elimina |
| Alles auswählen | Seleziona tutto |
| Suchen … / Weitersuchen / Rückwärts suchen | Trova… / Trova successivo / Trova precedente |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Contrai / Ridimensiona / Porta tutto in primo piano |
| Vollbildmodus | Attiva modalità a tutto schermo |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Nascondi la barra strumenti / Mostra/nascondi barra laterale |
| Sichern | Salva |
| Öffnen / Schließen | Apri / Chiudi |
| Abbrechen / OK | Annulla / OK |
| Im Finder zeigen | Mostra nel Finder |
| Papierkorb | Cestino (Sposta nel Cestino) |
| Schlüsselbund | Portachiavi |
| Zwischenablage | appunti (negli appunti) |
| Tab | pannello (as Safari names tabs) |
| AI-Tab | pannello AI |
| Panel | riquadro (pannello is the tab) |
| Seitenleiste | barra laterale |
| Vorschau | anteprima |
| Aktivität | Attività |
| Projekt / Ordner | progetto / cartella |
| Vertrauen, vertrauen | fiducia, fidarsi di (cartella attendibile) |
| Platte | disco (su disco) |
| Unterschiede | differenze |
| Zusammenführen | unire (unione) |
| Konflikt | conflitto |
| Treffer | risultato |
| Suche | Ricerca |
| Ersetzen | Sostituisci |
| Sprachserver | server di linguaggio |
| Hinweis (Schweregrad) | Suggerimento |
| Lesezeichen / Verlauf / Adresse | segnalibro / cronologia / indirizzo |
| Bytes | byte |
| Exportieren | Esporta |
| Format | Formato |
| Diktat | dettatura |
| Agent | agente |
| Eigener Agent | agente integrato |
| Eigene Agenten | Agenti personalizzati |
| Thread (AI-Thread, CLI-Thread) | thread (thread AI, thread CLI) |
| Sitzung | sessione |
| Unterhaltung | conversazione |
| Zug | turno |
| Antwort | risposta |
| Rückfrage | domanda |
| Freigabe, Freigabemodus | permesso, modalità permessi |
| Umgehen | Ignora |
| Erlauben / Ablehnen / Immer erlauben … | Consenti / Rifiuta / Consenti sempre… |
| Plan | piano |
| To-dos | Cose da fare |
| Aufgabe | task (attività is the Activity list) |
| Auftrag | incarico |
| Werkzeug | strumento |
| Skills | Skill |
| Befehl | comando |
| Hook | hook |
| Sandbox | sandbox |
| Fremder Inhalt | Contenuto esterno |
| Sicherungspunkt | punto di ripristino |
| Zurückspulen | riavvolgere |
| Verdichten | compattare |
| Kontext, Kontextfenster | contesto, finestra di contesto |
| Denktiefe | profondità di ragionamento |
| Modell / Anbieter / Endpunkt | modello / provider / endpoint |
| Token, Tokens | token |
| API-Schlüssel | chiave API |
| Register | registro |
| Adapter | adattatore |
| Bereit | Pronto |
| Prüfsumme | checksum |
| Anmeldung, anmelden | accesso, accedere |
| Konto | account |
| Abo | Abbonamento |
| Repository | repository |
| Commit, committen | commit, fare il commit |
| Secrets | segreti |
| vormerken | mettere in stage (togliere dallo stage) |
| Zurücknehmen | Togli dallo stage |
| nicht versioniert | non tracciato |
| Abschnitt (Git) | blocco |
| Branch / Remote | branch / remote |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Übernehmen (Worktree) | riprendi |
| Arbeitskopie | copia di lavoro |
| Lauf, Läufe | esecuzione, esecuzioni |
| Host | host |
| Job | job |
| Log | log |

### Japanese (`ja`)
- Address: no pronoun; explanations and questions in the polite です/ます form ("保存しますか?", "…してください"); menu items and
  buttons as plain verbs or nouns ("開く", "保存", "すべてを選択")
- Typography: “…” around names and interface words, as Japanese macOS ("“設定”で…"; not the 「…」 splat uses); full-width
  、。（） but half-width ? ! and ":" as macOS writes them ("アドレス:"); no space between Japanese and Latin text ("Finderに表示",
  "APIキー"); ellipsis "…" directly after the word ("設定…", "検索…"); katakana without a final long-vowel mark where Apple
  drops it (フォルダ, サーバ, プロバイダ)
- Git and CI words: katakana as Japanese developers write them: リポジトリ, コミット, ブランチ, リモート, プッシュ, プル, フェッチ, リベース, マージ,
  ステージ/ステージ解除, ハンク, ワークツリー, フック, ジョブ, ログ, トークン; kanji where Japanese Git tools use it: 競合 (conflict), 差分 (diff), 作業ツリー
  (working tree), 実行 (CI run); "AI" stays AI

| German key | Japanese |
|---|---|
| Ablage | ファイル |
| Bearbeiten | 編集 |
| Darstellung | 表示 |
| Fenster | ウインドウ |
| Hilfe | ヘルプ |
| Dienste | サービス |
| Über {} | {}について |
| Einstellungen | 設定 (menu item: 設定…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | {}を非表示 / ほかを非表示 / すべてを表示 |
| {} beenden | {}を終了 |
| Widerrufen / Wiederholen | 取り消す / やり直す |
| Ausschneiden / Kopieren / Einsetzen / Löschen | カット / コピー / ペースト / 削除 |
| Alles auswählen | すべてを選択 |
| Suchen … / Weitersuchen / Rückwärts suchen | 検索… / 次を検索 / 前を検索 |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | しまう / 拡大/縮小 / すべてを手前に移動 (拡大/縮小 as macOS; splat has 縮小/拡大) |
| Vollbildmodus | フルスクリーンにする |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | ツールバーを非表示 / サイドバーを切り替え |
| Sichern | 保存 |
| Öffnen / Schließen | 開く / 閉じる |
| Abbrechen / OK | キャンセル / OK |
| Im Finder zeigen | Finderに表示 |
| Papierkorb | ゴミ箱 (ゴミ箱に入れる) |
| Schlüsselbund | キーチェーン |
| Zwischenablage | クリップボード |
| Tab | タブ |
| AI-Tab | AIタブ |
| Panel | パネル |
| Seitenleiste | サイドバー |
| Vorschau | プレビュー |
| Aktivität | アクティビティ |
| Projekt / Ordner | プロジェクト / フォルダ |
| Vertrauen, vertrauen | 信頼, 信頼する (信頼済みのフォルダ) |
| Platte | ディスク (ディスク上の) |
| Unterschiede | 差分 |
| Zusammenführen | マージ |
| Konflikt | 競合 |
| Treffer | 結果 (検索結果) |
| Suche | 検索 |
| Ersetzen | 置換 |
| Sprachserver | 言語サーバ |
| Hinweis (Schweregrad) | ヒント |
| Lesezeichen / Verlauf / Adresse | ブックマーク / 履歴 / アドレス |
| Bytes | バイト |
| Exportieren | 書き出す |
| Format | フォーマット |
| Diktat | 音声入力 |
| Agent | エージェント |
| Eigener Agent | 内蔵エージェント |
| Eigene Agenten | カスタムエージェント |
| Thread (AI-Thread, CLI-Thread) | スレッド (AIスレッド, CLIスレッド) |
| Sitzung | セッション |
| Unterhaltung | 会話 |
| Zug | ターン |
| Antwort | 応答 |
| Rückfrage | 質問 |
| Freigabe, Freigabemodus | 権限, 権限モード |
| Umgehen | バイパス |
| Erlauben / Ablehnen / Immer erlauben … | 許可 / 拒否 / 常に許可… |
| Plan | プラン |
| To-dos | To-do |
| Aufgabe | タスク |
| Auftrag | タスク |
| Werkzeug | ツール |
| Skills | スキル |
| Befehl | コマンド |
| Hook | フック |
| Sandbox | サンドボックス |
| Fremder Inhalt | 外部コンテンツ |
| Sicherungspunkt | チェックポイント |
| Zurückspulen | 巻き戻し, 巻き戻す |
| Verdichten | 圧縮, 圧縮する |
| Kontext, Kontextfenster | コンテキスト, コンテキストウインドウ |
| Denktiefe | 思考の深さ |
| Modell / Anbieter / Endpunkt | モデル / プロバイダ / エンドポイント |
| Token, Tokens | トークン |
| API-Schlüssel | APIキー |
| Register | レジストリ |
| Adapter | アダプタ |
| Bereit | 準備完了 |
| Prüfsumme | チェックサム |
| Anmeldung, anmelden | サインイン, サインインする |
| Konto | アカウント |
| Abo | サブスクリプション |
| Repository | リポジトリ |
| Commit, committen | コミット, コミットする |
| Secrets | シークレット |
| vormerken | ステージする (ステージ解除) |
| Zurücknehmen | ステージ解除 |
| nicht versioniert | 未追跡 |
| Abschnitt (Git) | ハンク |
| Branch / Remote | ブランチ / リモート |
| Push / Pull / Fetch / Rebase | プッシュ / プル / フェッチ / リベース |
| Worktree | ワークツリー |
| Übernehmen (Worktree) | 取り込む |
| Arbeitskopie | 作業ツリー |
| Lauf, Läufe | 実行 |
| Host | ホスト |
| Job | ジョブ |
| Log | ログ |

### Korean (`ko`)
- Address: no pronoun; sentences in the formal 합쇼체, as Korean macOS ("저장하겠습니까?", "…하십시오", "…합니다"); menu items and
  buttons as nouns ("열기", "저장", "닫기")
- Typography: ‘…’ single quotation marks, as Korean macOS writes them (not the “…” splat uses); ellipsis "…" directly
  after the word, no space ("설정…", "찾기…"); a space between a Latin name and a following Korean noun ("AI 탭", "API 키"),
  particles attached ("Finder에서 보기", "{}에 관하여")
- Git and CI words: Hangul transliterations as Korean developers say them: 커밋, 브랜치, 푸시, 풀, 페치, 리베이스, 스테이징, 헝크, 워크트리, 훅,
  잡 (CI job), 로그, 토큰; Sino-Korean where Korean Git tools use it: 저장소 (repository), 원격 (remote), 병합 (merge), 충돌
  (conflict), 작업 트리 (working tree), 실행 (CI run); "AI" stays AI

| German key | Korean |
|---|---|
| Ablage | 파일 |
| Bearbeiten | 편집 |
| Darstellung | 보기 |
| Fenster | 윈도우 |
| Hilfe | 도움말 |
| Dienste | 서비스 |
| Über {} | {}에 관하여 |
| Einstellungen | 설정 (menu item: 설정…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | {} 가리기 / 기타 가리기 / 모두 보기 |
| {} beenden | {} 종료 |
| Widerrufen / Wiederholen | 실행 취소 / 실행 복귀 |
| Ausschneiden / Kopieren / Einsetzen / Löschen | 오려두기 / 복사하기 / 붙여넣기 / 삭제 |
| Alles auswählen | 전체 선택 |
| Suchen … / Weitersuchen / Rückwärts suchen | 찾기… / 다음 찾기 / 이전 찾기 |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | 최소화 / 확대/축소 / 모두 앞으로 가져오기 |
| Vollbildmodus | 전체 화면 시작 |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | 도구 막대 가리기 / 사이드바 토글 |
| Sichern | 저장 |
| Öffnen / Schließen | 열기 / 닫기 |
| Abbrechen / OK | 취소 / 확인 |
| Im Finder zeigen | Finder에서 보기 |
| Papierkorb | 휴지통 (휴지통으로 이동) |
| Schlüsselbund | 키체인 |
| Zwischenablage | 클립보드 |
| Tab | 탭 |
| AI-Tab | AI 탭 |
| Panel | 패널 |
| Seitenleiste | 사이드바 |
| Vorschau | 미리보기 |
| Aktivität | 활동 |
| Projekt / Ordner | 프로젝트 / 폴더 |
| Vertrauen, vertrauen | 신뢰, 신뢰하다 (신뢰하는 폴더) |
| Platte | 디스크 (디스크에 있는) |
| Unterschiede | 차이점 |
| Zusammenführen | 병합 |
| Konflikt | 충돌 |
| Treffer | 결과 |
| Suche | 검색 |
| Ersetzen | 대치 |
| Sprachserver | 언어 서버 |
| Hinweis (Schweregrad) | 힌트 |
| Lesezeichen / Verlauf / Adresse | 북마크 / 방문 기록 / 주소 |
| Bytes | 바이트 |
| Exportieren | 내보내기 |
| Format | 포맷 |
| Diktat | 받아쓰기 |
| Agent | 에이전트 |
| Eigener Agent | 내장 에이전트 |
| Eigene Agenten | 사용자 에이전트 |
| Thread (AI-Thread, CLI-Thread) | 스레드 (AI 스레드, CLI 스레드) |
| Sitzung | 세션 |
| Unterhaltung | 대화 |
| Zug | 턴 |
| Antwort | 응답 |
| Rückfrage | 질문 |
| Freigabe, Freigabemodus | 권한, 권한 모드 |
| Umgehen | 우회 |
| Erlauben / Ablehnen / Immer erlauben … | 허용 / 거부 / 항상 허용… |
| Plan | 계획 |
| To-dos | 할 일 |
| Aufgabe | 작업 |
| Auftrag | 작업 지시 |
| Werkzeug | 도구 |
| Skills | 스킬 |
| Befehl | 명령 |
| Hook | 훅 |
| Sandbox | 샌드박스 |
| Fremder Inhalt | 외부 콘텐츠 |
| Sicherungspunkt | 체크포인트 |
| Zurückspulen | 되감기 |
| Verdichten | 압축 |
| Kontext, Kontextfenster | 컨텍스트, 컨텍스트 윈도우 |
| Denktiefe | 사고 깊이 |
| Modell / Anbieter / Endpunkt | 모델 / 제공자 / 엔드포인트 |
| Token, Tokens | 토큰 |
| API-Schlüssel | API 키 |
| Register | 레지스트리 |
| Adapter | 어댑터 |
| Bereit | 준비됨 |
| Prüfsumme | 체크섬 |
| Anmeldung, anmelden | 로그인, 로그인하다 |
| Konto | 계정 |
| Abo | 구독 |
| Repository | 저장소 |
| Commit, committen | 커밋, 커밋하다 |
| Secrets | 시크릿 |
| vormerken | 스테이징하다 (스테이징 취소) |
| Zurücknehmen | 스테이징 취소 |
| nicht versioniert | 추적되지 않음 |
| Abschnitt (Git) | 헝크 |
| Branch / Remote | 브랜치 / 원격 |
| Push / Pull / Fetch / Rebase | 푸시 / 풀 / 페치 / 리베이스 |
| Worktree | 워크트리 |
| Übernehmen (Worktree) | 가져오기 |
| Arbeitskopie | 작업 트리 |
| Lauf, Läufe | 실행 |
| Host | 호스트 |
| Job | 잡 (작업 is the agent's task) |
| Log | 로그 |

### Malay (`ms`)
- Address: "anda" in lower case, as Malay macOS ("Adakah anda ingin…", "Buka pautan hanya jika anda mempercayai
  sumbernya"); menu items and buttons as imperative verbs ("Buka", "Simpan", "Tunjukkan")
- Typography: “…” quotation marks; ellipsis "…" directly after the word, no space ("Seting…", "Cari…"); menu items,
  buttons and titles in Title Case, as Malay macOS ("Tunjukkan Semua", "Pilih Semua"), running text in sentence case
- Git and CI words: kept as Malaysian developers say them: commit (buat commit), branch, remote, push, pull, fetch,
  rebase, stage/unstage, hunk, worktree, hook, job, log, token, endpoint, sandbox, checksum, registry; Malay where the
  word is settled: repositori, salinan kerja (working tree), gabung (merge), konflik, larian (CI run); "AI" stays AI

| German key | Malay |
|---|---|
| Ablage | Fail |
| Bearbeiten | Edit |
| Darstellung | Paparan |
| Fenster | Tetingkap |
| Hilfe | Bantuan |
| Dienste | Perkhidmatan |
| Über {} | Perihal {} |
| Einstellungen | Seting (menu item: Seting…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | Sembunyikan {} / Sembunyikan Lain / Tunjukkan Semua |
| {} beenden | Keluar daripada {} |
| Widerrufen / Wiederholen | Buat Asal / Buat Semula |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Potong / Salin / Tampal / Padam |
| Alles auswählen | Pilih Semua |
| Suchen … / Weitersuchen / Rückwärts suchen | Cari… / Cari Seterusnya / Cari Sebelumnya |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Minimumkan / Zum / Hantar Semua ke Depan Sekali |
| Vollbildmodus | Masuk Skrin Penuh |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Sembunyikan Bar Alat / Togol Bar Sisi |
| Sichern | Simpan |
| Öffnen / Schließen | Buka / Tutup |
| Abbrechen / OK | Batal / OK |
| Im Finder zeigen | Tunjukkan dalam Finder |
| Papierkorb | Sampah (Pindah ke Sampah) |
| Schlüsselbund | Rantai Kunci |
| Zwischenablage | papan klip |
| Tab | tab |
| AI-Tab | tab AI |
| Panel | panel |
| Seitenleiste | bar sisi |
| Vorschau | pratonton |
| Aktivität | Aktiviti |
| Projekt / Ordner | projek / folder |
| Vertrauen, vertrauen | percaya, mempercayai (folder yang dipercayai; button: Percaya) |
| Platte | cakera (pada cakera) |
| Unterschiede | perbezaan |
| Zusammenführen | gabung (penggabungan) |
| Konflikt | konflik |
| Treffer | hasil |
| Suche | Carian |
| Ersetzen | Ganti |
| Sprachserver | pelayan bahasa |
| Hinweis (Schweregrad) | Petunjuk |
| Lesezeichen / Verlauf / Adresse | penanda buku / sejarah / alamat |
| Bytes | bait |
| Exportieren | Eksport |
| Format | Format |
| Diktat | perencanaan (as Malay macOS names Dictation) |
| Agent | ejen |
| Eigener Agent | ejen terbina dalam |
| Eigene Agenten | Ejen tersuai |
| Thread (AI-Thread, CLI-Thread) | thread (thread AI, thread CLI) |
| Sitzung | sesi |
| Unterhaltung | perbualan |
| Zug | giliran |
| Antwort | jawapan |
| Rückfrage | soalan |
| Freigabe, Freigabemodus | keizinan, mod keizinan |
| Umgehen | Pintas |
| Erlauben / Ablehnen / Immer erlauben … | Benarkan / Tolak / Sentiasa Benarkan… |
| Plan | pelan |
| To-dos | Senarai Tugasan |
| Aufgabe | tugas |
| Auftrag | tugasan |
| Werkzeug | alat |
| Skills | Skill |
| Befehl | perintah |
| Hook | hook |
| Sandbox | sandbox |
| Fremder Inhalt | Kandungan Luaran |
| Sicherungspunkt | titik semak |
| Zurückspulen | putar balik |
| Verdichten | padatkan (pemadatan) |
| Kontext, Kontextfenster | konteks, tetingkap konteks |
| Denktiefe | kedalaman pemikiran |
| Modell / Anbieter / Endpunkt | model / penyedia / endpoint |
| Token, Tokens | token |
| API-Schlüssel | kunci API |
| Register | registry |
| Adapter | penyesuai |
| Bereit | Sedia |
| Prüfsumme | checksum |
| Anmeldung, anmelden | daftar masuk, daftar masuk ke |
| Konto | akaun |
| Abo | Langganan |
| Repository | repositori |
| Commit, committen | commit, buat commit |
| Secrets | rahsia |
| vormerken | stage (nyahstage) |
| Zurücknehmen | Nyahstage |
| nicht versioniert | tidak dijejak |
| Abschnitt (Git) | hunk |
| Branch / Remote | branch / remote |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Übernehmen (Worktree) | ambil alih |
| Arbeitskopie | salinan kerja |
| Lauf, Läufe | larian |
| Host | hos |
| Job | job |
| Log | log |

### Dutch (`nl`)
- Address: "je", as Dutch macOS ("Wil je…", "Weet je zeker dat…"); menu items and buttons as imperatives, verb first
  ("Bewaar", "Toon in Finder", "Verberg andere")
- Typography: straight single quotes '…' and the straight apostrophe ', as Dutch macOS writes them ("het document
  'Notities'", "foto's"; not the ‘…’ splat uses); ellipsis "…" directly after the word, no space ("Instellingen…",
  "Zoek…"); sentence case
- Git and CI words: kept as Dutch developers say them: repository, commit/committen, branch, remote, push, pull, fetch,
  rebase, stagen/unstagen, hunk, worktree, hook, job, log, run (CI), token, endpoint, checksum; Dutch where the word is
  settled: samenvoegen (merge), conflict, werkkopie (working tree), verschillen (diff); "AI" stays AI

| German key | Dutch |
|---|---|
| Ablage | Archief |
| Bearbeiten | Wijzig |
| Darstellung | Weergave |
| Fenster | Venster |
| Hilfe | Help |
| Dienste | Voorzieningen |
| Über {} | Over {} |
| Einstellungen | Instellingen (menu item: Instellingen…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | Verberg {} / Verberg andere / Toon alle |
| {} beenden | Stop {} |
| Widerrufen / Wiederholen | Herstel / Opnieuw |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Knip / Kopieer / Plak / Verwijder |
| Alles auswählen | Selecteer alles |
| Suchen … / Weitersuchen / Rückwärts suchen | Zoek… / Zoek volgende / Zoek vorige |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Minimaliseer / Vergroot/verklein / Alles op voorgrond |
| Vollbildmodus | Schakel schermvullende weergave in |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Verberg knoppenbalk / Toon/verberg navigatiekolom |
| Sichern | Bewaar |
| Öffnen / Schließen | Open / Sluit |
| Abbrechen / OK | Annuleer / OK |
| Im Finder zeigen | Toon in Finder |
| Papierkorb | prullenmand (Verplaats naar prullenmand) |
| Schlüsselbund | sleutelhanger |
| Zwischenablage | klembord |
| Tab | tabblad |
| AI-Tab | AI-tabblad |
| Panel | paneel |
| Seitenleiste | navigatiekolom |
| Vorschau | voorvertoning |
| Aktivität | Activiteit |
| Projekt / Ordner | project / map |
| Vertrauen, vertrauen | vertrouwen (vertrouwde map; button: Vertrouw) |
| Platte | schijf (op schijf) |
| Unterschiede | verschillen |
| Zusammenführen | samenvoegen (button: Voeg samen) |
| Konflikt | conflict |
| Treffer | resultaat |
| Suche | Zoeken |
| Ersetzen | Vervang |
| Sprachserver | taalserver |
| Hinweis (Schweregrad) | Hint |
| Lesezeichen / Verlauf / Adresse | bladwijzer / geschiedenis / adres |
| Bytes | bytes |
| Exportieren | Exporteer |
| Format | Opmaak |
| Diktat | dicteerfunctie (dicteren) |
| Agent | agent |
| Eigener Agent | ingebouwde agent |
| Eigene Agenten | Eigen agents |
| Thread (AI-Thread, CLI-Thread) | thread (AI-thread, CLI-thread) |
| Sitzung | sessie |
| Unterhaltung | gesprek |
| Zug | beurt |
| Antwort | antwoord |
| Rückfrage | vraag |
| Freigabe, Freigabemodus | toestemming, toestemmingsmodus |
| Umgehen | Omzeil |
| Erlauben / Ablehnen / Immer erlauben … | Sta toe / Weiger / Sta altijd toe… |
| Plan | plan |
| To-dos | To-do's |
| Aufgabe | taak |
| Auftrag | opdracht |
| Werkzeug | tool |
| Skills | Skills |
| Befehl | commando |
| Hook | hook |
| Sandbox | sandbox |
| Fremder Inhalt | Externe inhoud |
| Sicherungspunkt | herstelpunt |
| Zurückspulen | terugspoelen |
| Verdichten | compacteren |
| Kontext, Kontextfenster | context, contextvenster |
| Denktiefe | denkdiepte |
| Modell / Anbieter / Endpunkt | model / aanbieder / endpoint |
| Token, Tokens | token, tokens |
| API-Schlüssel | API-sleutel |
| Register | register |
| Adapter | adapter |
| Bereit | Gereed |
| Prüfsumme | checksum |
| Anmeldung, anmelden | inloggen (button: Log in) |
| Konto | account |
| Abo | Abonnement |
| Repository | repository |
| Commit, committen | commit, committen |
| Secrets | geheimen |
| vormerken | stagen (unstagen) |
| Zurücknehmen | Unstage |
| nicht versioniert | niet gevolgd |
| Abschnitt (Git) | hunk |
| Branch / Remote | branch / remote |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Übernehmen (Worktree) | overnemen |
| Arbeitskopie | werkkopie |
| Lauf, Läufe | run, runs |
| Host | host |
| Job | job |
| Log | log |

### Norwegian Bokmål (`no`)
- Address: "du", as Norwegian macOS ("Vil du…", "Er du sikker på at…"); menu items and buttons as imperatives ("Åpne",
  "Lagre", "Vis alle")
- Typography: «…» quotation marks; ellipsis "…" directly after the word, no space ("Innstillinger…", "Finn…"); sentence
  case
- Git and CI words: kept as Norwegian developers say them: repository, commit/committe, branch, remote, push, pull,
  fetch, rebase, stage/unstage, worktree, hook, token; Norwegian where the word is settled: slå sammen (merge),
  konflikt, blokk (hunk), arbeidskopi (working tree), kjøring (CI run), jobb, logg; "AI" is "KI", as Norwegian macOS
  writes it

| German key | Norwegian Bokmål |
|---|---|
| Ablage | Fil |
| Bearbeiten | Rediger |
| Darstellung | Vis |
| Fenster | Vindu |
| Hilfe | Hjelp |
| Dienste | Tjenester |
| Über {} | Om {} |
| Einstellungen | Innstillinger (menu item: Innstillinger…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | Skjul {} / Skjul andre / Vis alle |
| {} beenden | Avslutt {} |
| Widerrufen / Wiederholen | Angre / Utfør likevel |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Klipp ut / Kopier / Lim inn / Slett |
| Alles auswählen | Marker alt |
| Suchen … / Weitersuchen / Rückwärts suchen | Finn… / Finn neste / Finn forrige |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Minimer / Zoom / Legg alle øverst |
| Vollbildmodus | Gå til fullskjermmodus |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Skjul verktøylinje / Vis eller skjul sidepanel |
| Sichern | Lagre |
| Öffnen / Schließen | Åpne / Lukk |
| Abbrechen / OK | Avbryt / OK |
| Im Finder zeigen | Vis i Finder |
| Papierkorb | papirkurv (Flytt til papirkurv) |
| Schlüsselbund | nøkkelring |
| Zwischenablage | Utklipp (in running text: utklippstavlen) |
| Tab | fane |
| AI-Tab | KI-fane |
| Panel | panel |
| Seitenleiste | sidepanel |
| Vorschau | forhåndsvisning |
| Aktivität | Aktivitet |
| Projekt / Ordner | prosjekt / mappe |
| Vertrauen, vertrauen | tillit, stole på (mappe du stoler på; button: Stol på) |
| Platte | disk (på disken) |
| Unterschiede | forskjeller |
| Zusammenführen | slå sammen (sammenslåing) |
| Konflikt | konflikt |
| Treffer | treff |
| Suche | Søk |
| Ersetzen | Erstatt |
| Sprachserver | språktjener (macOS says tjener for server) |
| Hinweis (Schweregrad) | Tips |
| Lesezeichen / Verlauf / Adresse | bokmerke / logg (as Safari) / adresse |
| Bytes | byte |
| Exportieren | Eksporter |
| Format | Format |
| Diktat | diktering |
| Agent | agent |
| Eigener Agent | innebygd agent |
| Eigene Agenten | Egne agenter |
| Thread (AI-Thread, CLI-Thread) | tråd (KI-tråd, CLI-tråd) |
| Sitzung | økt |
| Unterhaltung | samtale |
| Zug | runde |
| Antwort | svar |
| Rückfrage | spørsmål |
| Freigabe, Freigabemodus | tillatelse, tillatelsesmodus |
| Umgehen | Omgå |
| Erlauben / Ablehnen / Immer erlauben … | Tillat / Avslå / Tillat alltid… |
| Plan | plan |
| To-dos | Gjøremål |
| Aufgabe | oppgave |
| Auftrag | oppdrag |
| Werkzeug | verktøy |
| Skills | Skills |
| Befehl | kommando |
| Hook | hook |
| Sandbox | sandkasse |
| Fremder Inhalt | Eksternt innhold |
| Sicherungspunkt | sjekkpunkt |
| Zurückspulen | spole tilbake |
| Verdichten | komprimere |
| Kontext, Kontextfenster | kontekst, kontekstvindu |
| Denktiefe | tenkedybde |
| Modell / Anbieter / Endpunkt | modell / leverandør / endepunkt |
| Token, Tokens | token, tokener |
| API-Schlüssel | API-nøkkel |
| Register | register |
| Adapter | adapter |
| Bereit | Klar |
| Prüfsumme | sjekksum |
| Anmeldung, anmelden | pålogging, logge på |
| Konto | konto |
| Abo | Abonnement |
| Repository | repository |
| Commit, committen | commit, committe |
| Secrets | hemmeligheter |
| vormerken | stage (unstage) |
| Zurücknehmen | Unstage |
| nicht versioniert | ikke sporet |
| Abschnitt (Git) | blokk |
| Branch / Remote | branch / remote |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Übernehmen (Worktree) | overta |
| Arbeitskopie | arbeidskopi |
| Lauf, Läufe | kjøring, kjøringer |
| Host | vert |
| Job | jobb |
| Log | logg |

### Polish (`pl`)
- Address: informal second person singular with the pronoun left out, as Polish macOS ("Czy chcesz…", "Otwieraj łącze
  tylko wtedy, gdy ufasz jego źródłu"); menu items and buttons as imperatives ("Otwórz", "Zachowaj", "Pokaż")
- Typography: „…” quotation marks; ellipsis "…" directly after the word, no space ("Ustawienia…", "Znajdź…"); sentence
  case; the About item is the bare app name with an ellipsis, as Polish macOS writes it ("{}…")
- Git and CI words: kept as Polish developers say them: commit (commitować, od ostatniego commitu), push, pull, fetch,
  rebase, worktree, hook, job, log, token; Polish where Polish Git tools (git-gui, VS Code) use it: repozytorium, gałąź
  (branch), repozytorium zdalne (remote), scalanie (merge), przygotuj/cofnij przygotowanie (stage/unstage), fragment
  (hunk), kopia robocza (working tree), uruchomienie (CI run), konflikt; "AI" stays AI

| German key | Polish |
|---|---|
| Ablage | Plik |
| Bearbeiten | Edycja |
| Darstellung | Widok |
| Fenster | Okno |
| Hilfe | Pomoc |
| Dienste | Usługi |
| Über {} | {}… |
| Einstellungen | Ustawienia (menu item: Ustawienia…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | Ukryj {} / Ukryj pozostałe / Pokaż wszystkie |
| {} beenden | Zakończ {} |
| Widerrufen / Wiederholen | Cofnij / Przywróć |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Wytnij / Kopiuj / Wklej / Usuń |
| Alles auswählen | Zaznacz wszystko |
| Suchen … / Weitersuchen / Rückwärts suchen | Znajdź… / Znajdź następne / Znajdź poprzednie |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Minimalizuj okno / Zoom / Umieść wszystko na wierzchu |
| Vollbildmodus | Tryb pełnoekranowy |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Ukryj pasek narzędzi / Przełącz pasek boczny |
| Sichern | Zachowaj |
| Öffnen / Schließen | Otwórz / Zamknij |
| Abbrechen / OK | Anuluj / OK |
| Im Finder zeigen | Pokaż w Finderze |
| Papierkorb | Kosz (Przenieś do Kosza) |
| Schlüsselbund | pęk kluczy |
| Zwischenablage | schowek |
| Tab | karta |
| AI-Tab | karta AI |
| Panel | panel |
| Seitenleiste | pasek boczny |
| Vorschau | podgląd |
| Aktivität | Aktywność |
| Projekt / Ordner | projekt / folder |
| Vertrauen, vertrauen | zaufanie, ufać (zaufany folder; button: Zaufaj) |
| Platte | dysk (na dysku) |
| Unterschiede | różnice |
| Zusammenführen | scalanie, scalić |
| Konflikt | konflikt |
| Treffer | wynik |
| Suche | Wyszukiwanie |
| Ersetzen | Zastąp |
| Sprachserver | serwer języka |
| Hinweis (Schweregrad) | Wskazówka |
| Lesezeichen / Verlauf / Adresse | zakładka / historia / adres |
| Bytes | bajty |
| Exportieren | Eksportuj |
| Format | Format |
| Diktat | dyktowanie |
| Agent | agent |
| Eigener Agent | wbudowany agent |
| Eigene Agenten | Własne agenty |
| Thread (AI-Thread, CLI-Thread) | wątek (wątek AI, wątek CLI) |
| Sitzung | sesja |
| Unterhaltung | rozmowa |
| Zug | tura |
| Antwort | odpowiedź |
| Rückfrage | pytanie |
| Freigabe, Freigabemodus | uprawnienie, tryb uprawnień |
| Umgehen | Pomiń |
| Erlauben / Ablehnen / Immer erlauben … | Pozwól / Nie pozwalaj / Zawsze pozwalaj… |
| Plan | plan |
| To-dos | Do zrobienia |
| Aufgabe | zadanie |
| Auftrag | zlecenie |
| Werkzeug | narzędzie |
| Skills | Umiejętności |
| Befehl | polecenie |
| Hook | hook |
| Sandbox | piaskownica |
| Fremder Inhalt | Treść zewnętrzna |
| Sicherungspunkt | punkt kontrolny |
| Zurückspulen | przewiń do tyłu (przewijanie do tyłu) |
| Verdichten | kompaktowanie, kompaktuj |
| Kontext, Kontextfenster | kontekst, okno kontekstu |
| Denktiefe | głębokość myślenia |
| Modell / Anbieter / Endpunkt | model / dostawca / punkt końcowy |
| Token, Tokens | token, tokeny |
| API-Schlüssel | klucz API |
| Register | rejestr |
| Adapter | adapter |
| Bereit | Gotowy |
| Prüfsumme | suma kontrolna |
| Anmeldung, anmelden | logowanie, zalogować się |
| Konto | konto |
| Abo | Subskrypcja |
| Repository | repozytorium |
| Commit, committen | commit, commitować |
| Secrets | sekrety |
| vormerken | przygotuj (cofnij przygotowanie) |
| Zurücknehmen | Cofnij przygotowanie |
| nicht versioniert | nieśledzony |
| Abschnitt (Git) | fragment |
| Branch / Remote | gałąź / repozytorium zdalne |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Übernehmen (Worktree) | przejmij |
| Arbeitskopie | kopia robocza |
| Lauf, Läufe | uruchomienie, uruchomienia |
| Host | host |
| Job | job |
| Log | log |

### Portuguese (Brazil) (`pt_BR`)
- Address: "você" with third-person verbs and "seu/sua", as Brazilian macOS ("Deseja tentar novamente?", "Abra o link
  somente se você confiar na fonte"); menu items and buttons as infinitives ("Abrir", "Salvar", "Mostrar Tudo")
- Typography: “…” quotation marks; ellipsis "…" directly after the word, no space ("Ajustes…", "Buscar…"); menu items,
  buttons and titles in Title Case, as Brazilian macOS ("Selecionar Tudo", "Mostrar Barra Lateral"), running text in
  sentence case
- Git and CI words: kept as Brazilian developers say them: commit (fazer commit), branch (a branch), push, pull, fetch,
  rebase, worktree, hook, job, log, token, endpoint, sandbox; Portuguese where the word is settled: repositório, remoto,
  mesclar (merge, as GitHub in Portuguese), preparar/remover da preparação (stage/unstage), bloco (hunk), cópia de
  trabalho (working tree), execução (CI run), conflito; "AI" is "IA"

| German key | Portuguese (Brazil) |
|---|---|
| Ablage | Arquivo |
| Bearbeiten | Editar |
| Darstellung | Visualizar |
| Fenster | Janela |
| Hilfe | Ajuda |
| Dienste | Serviços |
| Über {} | Sobre o {} |
| Einstellungen | Ajustes (menu item: Ajustes…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | Ocultar {} / Ocultar Outros / Mostrar Tudo |
| {} beenden | Encerrar {} |
| Widerrufen / Wiederholen | Desfazer / Refazer |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Cortar / Copiar / Colar / Apagar |
| Alles auswählen | Selecionar Tudo |
| Suchen … / Weitersuchen / Rückwärts suchen | Buscar… / Buscar Seguinte / Buscar Anterior |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Minimizar / Zoom / Trazer Todas para a Frente |
| Vollbildmodus | Entrar em Tela Cheia |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Ocultar Barra de Ferramentas / Mostrar/Ocultar Barra Lateral |
| Sichern | Salvar |
| Öffnen / Schließen | Abrir / Fechar |
| Abbrechen / OK | Cancelar / OK |
| Im Finder zeigen | Mostrar no Finder |
| Papierkorb | Lixo (Mover para o Lixo) |
| Schlüsselbund | Chaves (as macOS names the keychain: "nas Chaves") |
| Zwischenablage | área de transferência |
| Tab | aba |
| AI-Tab | aba de IA |
| Panel | painel |
| Seitenleiste | barra lateral |
| Vorschau | pré-visualização |
| Aktivität | Atividade |
| Projekt / Ordner | projeto / pasta |
| Vertrauen, vertrauen | confiança, confiar (pasta confiável; button: Confiar) |
| Platte | disco (no disco) |
| Unterschiede | diferenças |
| Zusammenführen | mesclar (mesclagem) |
| Konflikt | conflito |
| Treffer | resultado |
| Suche | Busca |
| Ersetzen | Substituir |
| Sprachserver | servidor de linguagem |
| Hinweis (Schweregrad) | Dica |
| Lesezeichen / Verlauf / Adresse | favorito (as Safari) / histórico / endereço |
| Bytes | bytes |
| Exportieren | Exportar |
| Format | Formatar |
| Diktat | ditado |
| Agent | agente |
| Eigener Agent | agente integrado |
| Eigene Agenten | Agentes personalizados |
| Thread (AI-Thread, CLI-Thread) | thread (thread de IA, thread de CLI) |
| Sitzung | sessão |
| Unterhaltung | conversa |
| Zug | turno |
| Antwort | resposta |
| Rückfrage | pergunta |
| Freigabe, Freigabemodus | permissão, modo de permissão |
| Umgehen | Ignorar |
| Erlauben / Ablehnen / Immer erlauben … | Permitir / Negar / Permitir Sempre… |
| Plan | plano |
| To-dos | Pendências |
| Aufgabe | tarefa |
| Auftrag | tarefa |
| Werkzeug | ferramenta |
| Skills | Skills |
| Befehl | comando |
| Hook | hook |
| Sandbox | sandbox |
| Fremder Inhalt | Conteúdo Externo |
| Sicherungspunkt | ponto de restauração |
| Zurückspulen | retroceder |
| Verdichten | compactar |
| Kontext, Kontextfenster | contexto, janela de contexto |
| Denktiefe | profundidade de raciocínio |
| Modell / Anbieter / Endpunkt | modelo / provedor / endpoint |
| Token, Tokens | token, tokens |
| API-Schlüssel | chave de API |
| Register | registro |
| Adapter | adaptador |
| Bereit | Pronto |
| Prüfsumme | soma de verificação |
| Anmeldung, anmelden | início de sessão, iniciar sessão |
| Konto | conta |
| Abo | Assinatura |
| Repository | repositório |
| Commit, committen | commit, fazer commit |
| Secrets | segredos |
| vormerken | preparar (remover da preparação) |
| Zurücknehmen | Remover da Preparação |
| nicht versioniert | não rastreado |
| Abschnitt (Git) | bloco |
| Branch / Remote | branch / remoto |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Übernehmen (Worktree) | incorporar |
| Arbeitskopie | cópia de trabalho |
| Lauf, Läufe | execução, execuções |
| Host | host |
| Job | job |
| Log | log |

### Portuguese (Portugal) (`pt_PT`)
- Address: formal third person with the pronoun left out, as European Portuguese macOS ("Pretende ignorar as
  alterações?", "Abra a hiperligação apenas se confiar na origem", "os seus"); menu items and buttons as infinitives
  ("Abrir", "Guardar", "Mostrar tudo")
- Typography: “…” quotation marks; ellipsis "…" directly after the word, no space ("Definições…", "Procurar…"); sentence
  case; European spelling and words as macOS uses them (ficheiro, ecrã, aplicação, hiperligação, registo)
- Git and CI words: kept as Portuguese developers say them: commit (fazer commit), branch, push, pull, fetch, rebase,
  worktree, hook, job, log, token, endpoint, sandbox, checksum; Portuguese where the word is settled: repositório,
  remoto, fundir (merge), preparar/anular preparação (stage/unstage), bloco (hunk), cópia de trabalho (working tree),
  execução (CI run), conflito; "AI" is "IA"

| German key | Portuguese (Portugal) |
|---|---|
| Ablage | Ficheiro |
| Bearbeiten | Edição |
| Darstellung | Visualização |
| Fenster | Janela |
| Hilfe | Ajuda |
| Dienste | Serviços |
| Über {} | Acerca do {} |
| Einstellungen | Definições (menu item: Definições…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | Ocultar o {} / Ocultar outras aplicações / Mostrar tudo |
| {} beenden | Sair do {} |
| Widerrufen / Wiederholen | Desfazer / Refazer |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Cortar / Copiar / Colar / Apagar |
| Alles auswählen | Selecionar tudo |
| Suchen … / Weitersuchen / Rückwärts suchen | Procurar… / Procurar seguinte / Procurar anterior |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Minimizar / Aumentar/reduzir / Passar tudo para a frente |
| Vollbildmodus | Ecrã completo |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Ocultar barra de ferramentas / Ocultar/mostrar barra lateral |
| Sichern | Guardar |
| Öffnen / Schließen | Abrir / Fechar |
| Abbrechen / OK | Cancelar / OK |
| Im Finder zeigen | Mostrar no Finder |
| Papierkorb | Lixo (Mover para o Lixo) |
| Schlüsselbund | Porta-chaves |
| Zwischenablage | clipboard (as European Portuguese macOS) |
| Tab | separador |
| AI-Tab | separador de IA |
| Panel | painel |
| Seitenleiste | barra lateral |
| Vorschau | pré-visualização |
| Aktivität | Atividade |
| Projekt / Ordner | projeto / pasta |
| Vertrauen, vertrauen | confiança, confiar (pasta de confiança; button: Confiar) |
| Platte | disco (no disco) |
| Unterschiede | diferenças |
| Zusammenführen | fundir (fusão) |
| Konflikt | conflito |
| Treffer | resultado |
| Suche | Pesquisa |
| Ersetzen | Substituir |
| Sprachserver | servidor de linguagem |
| Hinweis (Schweregrad) | Sugestão |
| Lesezeichen / Verlauf / Adresse | marcador / histórico / endereço |
| Bytes | bytes |
| Exportieren | Exportar |
| Format | Formatação |
| Diktat | ditado |
| Agent | agente |
| Eigener Agent | agente integrado |
| Eigene Agenten | Agentes personalizados |
| Thread (AI-Thread, CLI-Thread) | thread (thread de IA, thread de CLI) |
| Sitzung | sessão |
| Unterhaltung | conversa |
| Zug | turno |
| Antwort | resposta |
| Rückfrage | pergunta |
| Freigabe, Freigabemodus | permissão, modo de permissões |
| Umgehen | Ignorar |
| Erlauben / Ablehnen / Immer erlauben … | Permitir / Recusar / Permitir sempre… |
| Plan | plano |
| To-dos | Tarefas pendentes |
| Aufgabe | tarefa |
| Auftrag | tarefa |
| Werkzeug | ferramenta |
| Skills | Skills |
| Befehl | comando |
| Hook | hook |
| Sandbox | sandbox |
| Fremder Inhalt | Conteúdo externo |
| Sicherungspunkt | ponto de restauro |
| Zurückspulen | retroceder |
| Verdichten | compactar |
| Kontext, Kontextfenster | contexto, janela de contexto |
| Denktiefe | profundidade de raciocínio |
| Modell / Anbieter / Endpunkt | modelo / fornecedor / endpoint |
| Token, Tokens | token, tokens |
| API-Schlüssel | chave da API |
| Register | registo |
| Adapter | adaptador |
| Bereit | Pronto |
| Prüfsumme | checksum |
| Anmeldung, anmelden | início de sessão, iniciar sessão |
| Konto | conta |
| Abo | Subscrição |
| Repository | repositório |
| Commit, committen | commit, fazer commit |
| Secrets | segredos |
| vormerken | preparar (anular preparação) |
| Zurücknehmen | Anular preparação |
| nicht versioniert | não seguido |
| Abschnitt (Git) | bloco |
| Branch / Remote | branch / remoto |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Übernehmen (Worktree) | incorporar |
| Arbeitskopie | cópia de trabalho |
| Lauf, Läufe | execução, execuções |
| Host | anfitrião |
| Job | job |
| Log | log |

### Romanian (`ro`)
- Address: formal "dumneavoastră" (written "dvs."), as macOS Romanian; menu items, buttons and instructions in the
  second person plural imperative ("Salvați", "Anulați", "Introduceți…"), never "tu"
- Typography: „…” (U+201E, U+201D); ș and ț with comma below (U+0219, U+021B), never with cedilla; menu items end in "…"
  without a space ("Configurări…", "Salvați ca…"); sentence case
- Git and CI words: kept as Romanian developers say them, with Romanian endings: commit (commitul, a face commit),
  remote, push, pull, fetch, rebase, worktree, hook, sandbox, token (tokenuri), job (joburi), endpoint; Romanian where
  the word is settled: depozit (repository), ramură (branch), comasare (merge, as macOS says "Comasați"), a pregăti
  pentru commit (stage), fragment (hunk), copie de lucru (working tree), rulare (CI run), jurnal (log); "AI" is "IA", as
  macOS writes it

| German key | Romanian |
|---|---|
| Ablage | Fișier |
| Bearbeiten | Editare |
| Darstellung | Vizualizare |
| Fenster | Fereastră |
| Hilfe | Ajutor |
| Dienste | Servicii |
| Über {} | Despre {} |
| Einstellungen | Configurări (menu item: Configurări…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | Ascundeți {} / Ascundeți restul / Afișați tot |
| {} beenden | Închideți aplicația {} |
| Widerrufen / Wiederholen | Infirmați / Refaceți |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Tăiați / Copiați / Lipiți / Ștergeți |
| Alles auswählen | Selectați tot |
| Suchen … / Weitersuchen / Rückwärts suchen | Găsiți… / Găsiți următorul / Găsiți anteriorul |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Minimizați / Zoom / Aduceți tot în față |
| Vollbildmodus | Afișați pe tot ecranul |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Ascundeți bara de instrumente / Afișați/ascundeți bara laterală |
| Sichern | Salvați |
| Öffnen / Schließen | Deschideți / Închideți |
| Abbrechen / OK | Anulați / OK |
| Im Finder zeigen | Afișați în Finder |
| Papierkorb | Coș |
| Schlüsselbund | portchei |
| Zwischenablage | clipboard |
| Tab | filă |
| AI-Tab | filă IA |
| Panel | panou |
| Seitenleiste | bară laterală |
| Vorschau | previzualizare |
| Aktivität | Activitate |
| Projekt / Ordner | proiect / dosar |
| Vertrauen, vertrauen | încredere, a avea încredere în |
| Platte | disc (pe disc) |
| Unterschiede | diferențe |
| Zusammenführen | comasare, a comasa |
| Konflikt | conflict |
| Treffer | rezultat |
| Suche | Căutare |
| Ersetzen | Înlocuiți |
| Sprachserver | server de limbaj |
| Hinweis (Schweregrad) | Sugestie |
| Lesezeichen / Verlauf / Adresse | marcaj / istoric / adresă |
| Bytes | octeți |
| Exportieren | Exportați |
| Format | Format |
| Diktat | dictare |
| Agent | agent |
| Eigener Agent | agent propriu |
| Eigene Agenten | Agenți personalizați |
| Thread (AI-Thread, CLI-Thread) | conversație (conversație IA, conversație CLI) |
| Sitzung | sesiune |
| Unterhaltung | discuție |
| Zug | rundă |
| Antwort | răspuns |
| Rückfrage | întrebare |
| Freigabe, Freigabemodus | permisiune, mod de permisiuni |
| Umgehen | Ocolire |
| Erlauben / Ablehnen / Immer erlauben … | Permiteți / Refuzați / Permiteți întotdeauna… |
| Plan | plan |
| To-dos | De făcut |
| Aufgabe | sarcină |
| Auftrag | însărcinare |
| Werkzeug | instrument |
| Skills | Skilluri |
| Befehl | comandă |
| Hook | hook |
| Sandbox | sandbox |
| Fremder Inhalt | Conținut extern |
| Sicherungspunkt | punct de control |
| Zurückspulen | derulare înapoi, a derula înapoi |
| Verdichten | compactare, a compacta |
| Kontext, Kontextfenster | context, fereastră de context |
| Denktiefe | profunzimea gândirii |
| Modell / Anbieter / Endpunkt | model / furnizor / endpoint |
| Token, Tokens | token, tokenuri |
| API-Schlüssel | cheie API |
| Register | registru |
| Adapter | adaptor |
| Bereit | Gata |
| Prüfsumme | sumă de control |
| Anmeldung, anmelden | autentificare, a se autentifica |
| Konto | cont |
| Abo | Abonament |
| Repository | depozit |
| Commit, committen | commit, a face commit |
| Secrets | secrete |
| vormerken | a pregăti pentru commit |
| Zurücknehmen | Anulați pregătirea |
| nicht versioniert | neversionat |
| Abschnitt (Git) | fragment |
| Branch / Remote | ramură / remote |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Übernehmen (Worktree) | preluați |
| Arbeitskopie | copie de lucru |
| Lauf, Läufe | rulare, rulări |
| Host | gazdă |
| Job | job, joburi |
| Log | jurnal |

### Russian (`ru`)
- Address: formal "Вы", capitalized as macOS Russian writes it ("Вы", "Ваш"); menu items and buttons in the infinitive
  ("Сохранить", "Скрыть боковое меню"); instructions in the "Вы" imperative ("Выберите…", "Откройте…")
- Typography: «…» («„…“» inside); "е" instead of "ё", as macOS writes it ("учетная запись", "удаленный"); menu items end
  in "…" without a space ("Настройки…", "Сохранить как…"); dash with spaces " — "; the OK button is "ОК" in Cyrillic
  letters; sentence case
- Git and CI words: Git's operation names stay in Latin as Russian developers write them: push, pull, fetch, rebase,
  worktree; Russian otherwise: репозиторий, коммит (закоммитить), ветка (branch), удаленный репозиторий (remote),
  индексировать (stage), фрагмент (hunk), слияние (merge), рабочая копия, хук, песочница, токен, запуск (CI run),
  задание (job), журнал (log); "AI" is "ИИ", as macOS writes it

| German key | Russian |
|---|---|
| Ablage | Файл |
| Bearbeiten | Правка |
| Darstellung | Вид |
| Fenster | Окно |
| Hilfe | Справка |
| Dienste | Службы |
| Über {} | О приложении {} |
| Einstellungen | Настройки (menu item: Настройки…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | Скрыть {} / Скрыть остальные / Показать все |
| {} beenden | Завершить {} |
| Widerrufen / Wiederholen | Отменить / Повторить |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Вырезать / Скопировать / Вставить / Удалить |
| Alles auswählen | Выбрать все |
| Suchen … / Weitersuchen / Rückwärts suchen | Найти… / Найти далее / Найти ранее |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Убрать в Dock / Изменить масштаб / Все окна — на передний план |
| Vollbildmodus | Перейти в полноэкранный режим |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Скрыть панель инструментов / Показать/скрыть боковое меню |
| Sichern | Сохранить |
| Öffnen / Schließen | Открыть / Закрыть |
| Abbrechen / OK | Отменить / ОК |
| Im Finder zeigen | Показать в Finder |
| Papierkorb | Корзина |
| Schlüsselbund | связка ключей |
| Zwischenablage | буфер обмена |
| Tab | вкладка |
| AI-Tab | вкладка ИИ |
| Panel | панель |
| Seitenleiste | боковое меню |
| Vorschau | просмотр |
| Aktivität | Активность |
| Projekt / Ordner | проект / папка |
| Vertrauen, vertrauen | доверие, доверять |
| Platte | диск (на диске) |
| Unterschiede | различия |
| Zusammenführen | слияние, слить |
| Konflikt | конфликт |
| Treffer | результат |
| Suche | Поиск |
| Ersetzen | Заменить |
| Sprachserver | языковой сервер |
| Hinweis (Schweregrad) | Подсказка |
| Lesezeichen / Verlauf / Adresse | закладка / история / адрес |
| Bytes | байт |
| Exportieren | Экспортировать |
| Format | Формат |
| Diktat | диктовка |
| Agent | агент |
| Eigener Agent | собственный агент |
| Eigene Agenten | Пользовательские агенты |
| Thread (AI-Thread, CLI-Thread) | чат (ИИ-чат, CLI-чат) |
| Sitzung | сеанс |
| Unterhaltung | диалог |
| Zug | запрос |
| Antwort | ответ |
| Rückfrage | вопрос |
| Freigabe, Freigabemodus | разрешение, режим разрешений |
| Umgehen | Обход разрешений |
| Erlauben / Ablehnen / Immer erlauben … | Разрешить / Запретить / Разрешать всегда… |
| Plan | план |
| To-dos | Список дел |
| Aufgabe | задача |
| Auftrag | поручение |
| Werkzeug | инструмент |
| Skills | Навыки |
| Befehl | команда |
| Hook | хук |
| Sandbox | песочница |
| Fremder Inhalt | Внешнее содержимое |
| Sicherungspunkt | контрольная точка |
| Zurückspulen | откат, откатить |
| Verdichten | сжатие, сжать |
| Kontext, Kontextfenster | контекст, контекстное окно |
| Denktiefe | глубина рассуждений |
| Modell / Anbieter / Endpunkt | модель / провайдер / конечная точка |
| Token, Tokens | токен, токены |
| API-Schlüssel | ключ API |
| Register | реестр |
| Adapter | адаптер |
| Bereit | Готов |
| Prüfsumme | контрольная сумма |
| Anmeldung, anmelden | вход, войти |
| Konto | учетная запись |
| Abo | Подписка |
| Repository | репозиторий |
| Commit, committen | коммит, закоммитить |
| Secrets | секреты |
| vormerken | индексировать |
| Zurücknehmen | Убрать из индекса |
| nicht versioniert | не отслеживается |
| Abschnitt (Git) | фрагмент |
| Branch / Remote | ветка / удаленный репозиторий |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Übernehmen (Worktree) | перенести |
| Arbeitskopie | рабочая копия |
| Lauf, Läufe | запуск, запуски |
| Host | хост |
| Job | задание |
| Log | журнал |

### Slovak (`sk`)
- Address: formal "vy" (vykanie), lowercase; menu items and buttons in the infinitive, as macOS Slovak has them
  ("Uložiť", "Zobraziť postranný panel"); instructions in the plural imperative ("Vyberte…", "Otvorte…"); help tags in
  the third person ("Zobrazí alebo skryje…")
- Typography: „…“ (U+201E, U+201C); menu items end in "…" without a space ("Nastavenia…", "Uložiť ako…"); the OK button
  is "Potvrdiť", as macOS Slovak writes it; sentence case
- Git and CI words: kept as Slovak developers say them and declined: commit (commitnúť, commitu), push, pull, fetch,
  rebase, worktree, hook, sandbox, token (tokeny), job (joby), log; Slovak where the word is settled: repozitár, vetva
  (branch), vzdialený repozitár (remote), zlúčiť/zlúčenie (merge), pripraviť na commit (stage), úsek (hunk), pracovná
  kópia, spustenie (CI run); "AI" stays "AI"

| German key | Slovak |
|---|---|
| Ablage | Súbor |
| Bearbeiten | Upraviť |
| Darstellung | Zobraziť |
| Fenster | Okno |
| Hilfe | Pomocník |
| Dienste | Služby |
| Über {} | O aplikácii {} |
| Einstellungen | Nastavenia (menu item: Nastavenia…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | Skryť {} / Skryť ostatné / Zobraziť všetky |
| {} beenden | Ukončiť {} |
| Widerrufen / Wiederholen | Odvolať / Obnoviť |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Vystrihnúť / Kopírovať / Vložiť / Vymazať |
| Alles auswählen | Označiť všetko |
| Suchen … / Weitersuchen / Rückwärts suchen | Nájsť… / Nájsť ďalšie / Nájsť predošlé |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Minimalizovať / Zmeniť veľkosť / Presunúť všetky do popredia |
| Vollbildmodus | Zobraziť na celej obrazovke |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Skryť panel s nástrojmi / Zobraziť/skryť postranný panel |
| Sichern | Uložiť |
| Öffnen / Schließen | Otvoriť / Zatvoriť |
| Abbrechen / OK | Zrušiť / Potvrdiť |
| Im Finder zeigen | Zobraziť vo Finderi |
| Papierkorb | Kôš |
| Schlüsselbund | kľúčenka |
| Zwischenablage | schránka |
| Tab | tab |
| AI-Tab | AI tab |
| Panel | panel |
| Seitenleiste | postranný panel |
| Vorschau | náhľad |
| Aktivität | Aktivita |
| Projekt / Ordner | projekt / priečinok |
| Vertrauen, vertrauen | dôvera, dôverovať |
| Platte | disk (na disku) |
| Unterschiede | rozdiely |
| Zusammenführen | zlúčenie, zlúčiť |
| Konflikt | konflikt |
| Treffer | výsledok |
| Suche | Vyhľadávanie |
| Ersetzen | Nahradiť |
| Sprachserver | jazykový server |
| Hinweis (Schweregrad) | Nápoveda |
| Lesezeichen / Verlauf / Adresse | záložka / história / adresa |
| Bytes | B |
| Exportieren | Exportovať |
| Format | Formát |
| Diktat | diktovanie |
| Agent | agent |
| Eigener Agent | vlastný agent |
| Eigene Agenten | Vlastní agenti |
| Thread (AI-Thread, CLI-Thread) | vlákno (AI vlákno, CLI vlákno) |
| Sitzung | relácia |
| Unterhaltung | konverzácia |
| Zug | kolo |
| Antwort | odpoveď |
| Rückfrage | otázka |
| Freigabe, Freigabemodus | povolenie, režim povolení |
| Umgehen | Obísť |
| Erlauben / Ablehnen / Immer erlauben … | Povoliť / Zamietnuť / Vždy povoliť… |
| Plan | plán |
| To-dos | Zoznam úloh |
| Aufgabe | úloha |
| Auftrag | zadanie |
| Werkzeug | nástroj |
| Skills | Skills |
| Befehl | príkaz |
| Hook | hook |
| Sandbox | sandbox |
| Fremder Inhalt | Externý obsah |
| Sicherungspunkt | kontrolný bod |
| Zurückspulen | pretočiť späť |
| Verdichten | zhustiť, zhustenie |
| Kontext, Kontextfenster | kontext, kontextové okno |
| Denktiefe | hĺbka uvažovania |
| Modell / Anbieter / Endpunkt | model / poskytovateľ / koncový bod |
| Token, Tokens | token, tokeny |
| API-Schlüssel | kľúč API |
| Register | register |
| Adapter | adaptér |
| Bereit | Pripravené |
| Prüfsumme | kontrolný súčet |
| Anmeldung, anmelden | prihlásenie, prihlásiť sa |
| Konto | účet |
| Abo | Predplatné |
| Repository | repozitár |
| Commit, committen | commit, commitnúť |
| Secrets | tajné údaje |
| vormerken | pripraviť na commit |
| Zurücknehmen | Odobrať z prípravy |
| nicht versioniert | nesledované |
| Abschnitt (Git) | úsek |
| Branch / Remote | vetva / vzdialený repozitár |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Übernehmen (Worktree) | prevziať |
| Arbeitskopie | pracovná kópia |
| Lauf, Läufe | spustenie, spustenia |
| Host | hostiteľ |
| Job | job |
| Log | log |

### Slovenian (`sl`)
- Address: formal "vi" (vikanje), lowercase; menu items and buttons in the singular imperative, as macOS Slovenian has
  them ("Shrani", "Prikaži stransko vrstico"); instructions in the plural ("Izberite…", "Odprite…"); questions as
  "Želite …?"
- Typography: »…« ; menu items put a no-break space (U+00A0) before "…", as macOS writes it ("Nastavitve …", "Shrani
  kot …"; splat has a plain space there); the OK button is "V redu"; sentence case
- Git and CI words: kept as Slovenian developers say them and declined: commit (commita, narediti commit), push, pull,
  fetch, rebase, worktree, hook, job; Slovenian where the word is settled: repozitorij, veja (branch), oddaljeni
  repozitorij (remote), združiti/združitev (merge), pripraviti za commit (stage), odsek (hunk), delovna kopija, zagon
  (CI run), dnevnik (log), peskovnik (sandbox), žeton (token); "AI" stays "AI" (macOS spells out "umetna inteligenca";
  the short form "UI" would read as user interface)

| German key | Slovenian |
|---|---|
| Ablage | Datoteka |
| Bearbeiten | Uredi |
| Darstellung | Prikaz |
| Fenster | Okno |
| Hilfe | Pomoč |
| Dienste | Storitve |
| Über {} | O aplikaciji {} |
| Einstellungen | Nastavitve (menu item: Nastavitve …) |
| {} ausblenden / Andere ausblenden / Alle einblenden | Skrij aplikacijo {} / Skrij druge / Prikaži vse |
| {} beenden | Zapri aplikacijo {} |
| Widerrufen / Wiederholen | Razveljavi / Uveljavi |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Izreži / Kopiraj / Prilepi / Izbriši |
| Alles auswählen | Izberi vse |
| Suchen … / Weitersuchen / Rückwärts suchen | Poišči … / Poišči naslednje / Poišči prejšnje |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Minimiraj / Povečaj / Postavi vse v ospredje |
| Vollbildmodus | Preklopi v celozaslonski način |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Skrij orodno vrstico / Prikaži/skrij stransko vrstico |
| Sichern | Shrani |
| Öffnen / Schließen | Odpri / Zapri |
| Abbrechen / OK | Prekliči / V redu |
| Im Finder zeigen | Prikaži v aplikaciji Finder |
| Papierkorb | Koš |
| Schlüsselbund | veriga ključev |
| Zwischenablage | odložišče |
| Tab | zavihek |
| AI-Tab | zavihek AI |
| Panel | podokno |
| Seitenleiste | stranska vrstica |
| Vorschau | predogled |
| Aktivität | Aktivnost |
| Projekt / Ordner | projekt / mapa |
| Vertrauen, vertrauen | zaupanje, zaupati |
| Platte | disk (na disku) |
| Unterschiede | razlike |
| Zusammenführen | združitev, združiti |
| Konflikt | konflikt |
| Treffer | rezultat |
| Suche | Iskanje |
| Ersetzen | Zamenjaj |
| Sprachserver | jezikovni strežnik |
| Hinweis (Schweregrad) | Namig |
| Lesezeichen / Verlauf / Adresse | zaznamek / zgodovina / naslov |
| Bytes | bajti |
| Exportieren | Izvozi |
| Format | Oblikovanje |
| Diktat | narekovanje |
| Agent | agent |
| Eigener Agent | lastni agent |
| Eigene Agenten | Agenti po meri |
| Thread (AI-Thread, CLI-Thread) | nit (nit AI, nit CLI) |
| Sitzung | seja |
| Unterhaltung | pogovor |
| Zug | krog |
| Antwort | odgovor |
| Rückfrage | vprašanje |
| Freigabe, Freigabemodus | dovoljenje, način dovoljenj |
| Umgehen | Obidi |
| Erlauben / Ablehnen / Immer erlauben … | Dovoli / Zavrni / Vedno dovoli … |
| Plan | načrt |
| To-dos | Opravki |
| Aufgabe | opravilo |
| Auftrag | naročilo |
| Werkzeug | orodje |
| Skills | Skills |
| Befehl | ukaz |
| Hook | hook |
| Sandbox | peskovnik |
| Fremder Inhalt | Zunanja vsebina |
| Sicherungspunkt | kontrolna točka |
| Zurückspulen | previti nazaj, previjanje nazaj |
| Verdichten | zgostiti, zgoščevanje |
| Kontext, Kontextfenster | kontekst, kontekstno okno |
| Denktiefe | globina razmisleka |
| Modell / Anbieter / Endpunkt | model / ponudnik / končna točka |
| Token, Tokens | žeton, žetoni |
| API-Schlüssel | ključ API |
| Register | register |
| Adapter | adapter |
| Bereit | Pripravljeno |
| Prüfsumme | kontrolna vsota |
| Anmeldung, anmelden | vpis, vpisati se |
| Konto | račun |
| Abo | Naročnina |
| Repository | repozitorij |
| Commit, committen | commit, narediti commit |
| Secrets | skrivnosti |
| vormerken | pripraviti za commit |
| Zurücknehmen | Prekliči pripravo |
| nicht versioniert | nesledeno |
| Abschnitt (Git) | odsek |
| Branch / Remote | veja / oddaljeni repozitorij |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Übernehmen (Worktree) | prevzemi |
| Arbeitskopie | delovna kopija |
| Lauf, Läufe | zagon, zagoni |
| Host | gostitelj |
| Job | job |
| Log | dnevnik |

### Swedish (`sv`)
- Address: "du", as macOS Swedish ("Vill du …?", "Välj …"); menu items and buttons in the imperative ("Spara", "Göm
  verktygsfältet"); "Göm", not "Dölj", for hide, as macOS says it
- Typography: ”…” (U+201D on both sides) and ’; menu items end in "…" without a space ("Inställningar…", "Spara som…");
  sentence case
- Git and CI words: kept as Swedish developers say them, with Swedish endings: commit (committa, commiten), stagea, repo
  (repot), remote (remoten), push, pull, fetch, rebase, worktree, hook, token; Swedish where the word is settled: gren
  (branch), slå ihop/sammanslagning (merge, as macOS says "Slå ihop"), ändringsblock (hunk), arbetskopia (working tree),
  körning (CI run), jobb (job), logg (log), sandlåda (sandbox); "AI" stays "AI"

| German key | Swedish |
|---|---|
| Ablage | Arkiv |
| Bearbeiten | Redigera |
| Darstellung | Innehåll |
| Fenster | Fönster |
| Hilfe | Hjälp |
| Dienste | Tjänster |
| Über {} | Om {} |
| Einstellungen | Inställningar (menu item: Inställningar…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | Göm {} / Göm övriga / Visa alla |
| {} beenden | Avsluta {} |
| Widerrufen / Wiederholen | Ångra / Gör om |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Klipp ut / Kopiera / Klistra in / Radera |
| Alles auswählen | Markera allt |
| Suchen … / Weitersuchen / Rückwärts suchen | Sök… / Sök nästa / Sök föregående |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Minimera / Zooma / Lägg alla överst |
| Vollbildmodus | Helskärmsläge |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Göm verktygsfältet / Visa/göm sidofältet |
| Sichern | Spara |
| Öffnen / Schließen | Öppna / Stäng |
| Abbrechen / OK | Avbryt / OK |
| Im Finder zeigen | Visa i Finder |
| Papierkorb | papperskorg (papperskorgen) |
| Schlüsselbund | nyckelring |
| Zwischenablage | urklipp |
| Tab | flik |
| AI-Tab | AI-flik |
| Panel | panel |
| Seitenleiste | sidofält |
| Vorschau | förhandsvisning |
| Aktivität | Aktivitet |
| Projekt / Ordner | projekt / mapp |
| Vertrauen, vertrauen | förtroende, lita på |
| Platte | disk (på disken) |
| Unterschiede | skillnader |
| Zusammenführen | slå ihop, sammanslagning |
| Konflikt | konflikt |
| Treffer | träff |
| Suche | Sök |
| Ersetzen | Ersätt |
| Sprachserver | språkserver |
| Hinweis (Schweregrad) | Tips |
| Lesezeichen / Verlauf / Adresse | bokmärke / historik / adress |
| Bytes | byte |
| Exportieren | Exportera |
| Format | Format |
| Diktat | diktering |
| Agent | agent |
| Eigener Agent | egen agent |
| Eigene Agenten | Anpassade agenter |
| Thread (AI-Thread, CLI-Thread) | tråd (AI-tråd, CLI-tråd) |
| Sitzung | session |
| Unterhaltung | konversation |
| Zug | omgång |
| Antwort | svar |
| Rückfrage | fråga |
| Freigabe, Freigabemodus | behörighet, behörighetsläge |
| Umgehen | Kringgå |
| Erlauben / Ablehnen / Immer erlauben … | Tillåt / Neka / Tillåt alltid… |
| Plan | plan |
| To-dos | Att göra |
| Aufgabe | uppgift |
| Auftrag | uppdrag |
| Werkzeug | verktyg |
| Skills | Skills |
| Befehl | kommando |
| Hook | hook |
| Sandbox | sandlåda |
| Fremder Inhalt | Externt innehåll |
| Sicherungspunkt | kontrollpunkt |
| Zurückspulen | spola tillbaka |
| Verdichten | komprimera, komprimering |
| Kontext, Kontextfenster | kontext, kontextfönster |
| Denktiefe | tankedjup |
| Modell / Anbieter / Endpunkt | modell / leverantör / slutpunkt |
| Token, Tokens | token, tokens |
| API-Schlüssel | API-nyckel |
| Register | register |
| Adapter | adapter |
| Bereit | Redo |
| Prüfsumme | kontrollsumma |
| Anmeldung, anmelden | inloggning, logga in |
| Konto | konto |
| Abo | Abonnemang |
| Repository | repo |
| Commit, committen | commit, committa |
| Secrets | hemligheter |
| vormerken | stagea |
| Zurücknehmen | Unstagea |
| nicht versioniert | ospårad |
| Abschnitt (Git) | ändringsblock |
| Branch / Remote | gren / remote |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Übernehmen (Worktree) | ta över |
| Arbeitskopie | arbetskopia |
| Lauf, Läufe | körning, körningar |
| Host | värd |
| Job | jobb |
| Log | logg |

### Thai (`th`)
- Address: polite neutral "คุณ", as macOS Thai; menu items and buttons are bare verbs ("บันทึก", "ซ่อนแถบเครื่องมือ");
  no politeness particles (ครับ/ค่ะ)
- Typography: “…”; menu items end in "…" without a space ("การตั้งค่า…", "บันทึกเป็น…"); no full stop at the end of a
  sentence, a space separates sentences; a space between Thai and Latin words or a placeholder ("แสดงใน Finder", "ออกจาก
  {}")
- Git and CI words: Git's words stay in Latin script and capitalized like names, as Thai developers write them and as
  splat does ("Commit"): Repository, Commit, Stage, Branch, Remote, Push, Pull, Fetch, Rebase, Worktree, Hook, Job; Thai
  otherwise: ผสาน (merge), ความขัดแย้ง (conflict), ส่วนที่เปลี่ยนแปลง (hunk), สำเนาที่ใช้งาน (working tree), การรัน (CI
  run), ล็อก (log); "AI" stays "AI"

| German key | Thai |
|---|---|
| Ablage | ไฟล์ |
| Bearbeiten | แก้ไข |
| Darstellung | มุมมอง |
| Fenster | หน้าต่าง |
| Hilfe | วิธีใช้ |
| Dienste | บริการ |
| Über {} | เกี่ยวกับ {} |
| Einstellungen | การตั้งค่า (menu item: การตั้งค่า…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | ซ่อน {} / ซ่อนอื่นๆ / แสดงทั้งหมด |
| {} beenden | ออกจาก {} |
| Widerrufen / Wiederholen | เลิกทำ / ทำกลับมาใหม่ |
| Ausschneiden / Kopieren / Einsetzen / Löschen | ตัด / คัดลอก / วาง / ลบ |
| Alles auswählen | เลือกทั้งหมด |
| Suchen … / Weitersuchen / Rückwärts suchen | ค้นหา… / ค้นหาถัดไป / ค้นหาก่อนหน้า |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | ย่อ / ซูม / นำทั้งหมดมาด้านหน้า |
| Vollbildmodus | เข้าสู่โหมดเต็มหน้าจอ |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | ซ่อนแถบเครื่องมือ / แสดง/ซ่อนแถบด้านข้าง |
| Sichern | บันทึก |
| Öffnen / Schließen | เปิด / ปิด |
| Abbrechen / OK | ยกเลิก / ตกลง |
| Im Finder zeigen | แสดงใน Finder |
| Papierkorb | ถังขยะ |
| Schlüsselbund | พวงกุญแจ |
| Zwischenablage | คลิปบอร์ด |
| Tab | แถบ |
| AI-Tab | แถบ AI |
| Panel | แผง |
| Seitenleiste | แถบด้านข้าง |
| Vorschau | แสดงตัวอย่าง |
| Aktivität | กิจกรรม |
| Projekt / Ordner | โปรเจ็กต์ / โฟลเดอร์ |
| Vertrauen, vertrauen | ความเชื่อถือ, เชื่อถือ |
| Platte | ดิสก์ (บนดิสก์) |
| Unterschiede | ความแตกต่าง |
| Zusammenführen | ผสาน, การผสาน |
| Konflikt | ความขัดแย้ง |
| Treffer | ผลการค้นหา |
| Suche | ค้นหา |
| Ersetzen | แทนที่ |
| Sprachserver | เซิร์ฟเวอร์ภาษา |
| Hinweis (Schweregrad) | คำแนะนำ |
| Lesezeichen / Verlauf / Adresse | ที่คั่นหน้า / ประวัติ / ที่อยู่ |
| Bytes | ไบต์ |
| Exportieren | ส่งออก |
| Format | รูปแบบ |
| Diktat | การป้อนตามคำบอก |
| Agent | เอเจนต์ |
| Eigener Agent | เอเจนต์ในตัว |
| Eigene Agenten | เอเจนต์ที่กำหนดเอง |
| Thread (AI-Thread, CLI-Thread) | เธรด (เธรด AI, เธรด CLI) |
| Sitzung | เซสชัน |
| Unterhaltung | บทสนทนา |
| Zug | รอบ |
| Antwort | คำตอบ |
| Rückfrage | คำถาม |
| Freigabe, Freigabemodus | สิทธิ์, โหมดสิทธิ์ |
| Umgehen | ข้ามสิทธิ์ |
| Erlauben / Ablehnen / Immer erlauben … | อนุญาต / ปฏิเสธ / อนุญาตเสมอ… |
| Plan | แผน |
| To-dos | สิ่งที่ต้องทำ |
| Aufgabe | งาน |
| Auftrag | งานที่มอบหมาย |
| Werkzeug | เครื่องมือ |
| Skills | Skills |
| Befehl | คำสั่ง |
| Hook | Hook |
| Sandbox | แซนด์บ็อกซ์ |
| Fremder Inhalt | เนื้อหาจากภายนอก |
| Sicherungspunkt | จุดตรวจสอบ |
| Zurückspulen | กรอกลับ |
| Verdichten | บีบอัด |
| Kontext, Kontextfenster | บริบท, หน้าต่างบริบท |
| Denktiefe | ระดับการคิด |
| Modell / Anbieter / Endpunkt | โมเดล / ผู้ให้บริการ / เอนด์พอยต์ |
| Token, Tokens | โทเค็น |
| API-Schlüssel | คีย์ API |
| Register | รีจิสทรี |
| Adapter | อะแดปเตอร์ |
| Bereit | พร้อม |
| Prüfsumme | เช็คซัม |
| Anmeldung, anmelden | การลงชื่อเข้า, ลงชื่อเข้า |
| Konto | บัญชี |
| Abo | การสมัครใช้บริการ |
| Repository | Repository |
| Commit, committen | Commit, ทำ Commit |
| Secrets | ข้อมูลลับ |
| vormerken | Stage |
| Zurücknehmen | Unstage |
| nicht versioniert | ไม่ได้ติดตาม |
| Abschnitt (Git) | ส่วนที่เปลี่ยนแปลง |
| Branch / Remote | Branch / Remote |
| Push / Pull / Fetch / Rebase | Push / Pull / Fetch / Rebase |
| Worktree | Worktree |
| Übernehmen (Worktree) | นำมาใช้ |
| Arbeitskopie | สำเนาที่ใช้งาน |
| Lauf, Läufe | การรัน |
| Host | โฮสต์ |
| Job | Job |
| Log | ล็อก |

### Turkish (`tr`)
- Address: formal "siz", as macOS Turkish ("… istediğinizden emin misiniz?", "Seçin"); menu items and buttons in the
  bare imperative ("Kaydet", "Kenar Çubuğunu Göster"); yes/no questions in the passive ("… kaydedilsin mi?")
- Typography: “…”; a suffix after a name or Latin word is joined with ’ ("Finder’da", "commit’ler"); where a suffix
  would have to follow a placeholder, rephrase with a colon as macOS does ("Gizle: {}", "Çık: {}"); menu items end in
  "…" without a space ("Ayarlar…", "Farklı Kaydet…"); menu items and buttons in title case, as macOS Turkish writes them
  ("Tümünü Seç", "Araç Çubuğunu Gizle"), sentences in sentence case
- Git and CI words: kept as Turkish developers say them: commit (commit etmek), push, pull, fetch, rebase, worktree,
  hook, token; Turkish where the word is settled: depo (repository), dal (branch), uzak depo (remote),
  birleştir/birleştirme (merge), çakışma (conflict), hazırla (stage), parça (hunk), çalışma kopyası, çalıştırma (CI
  run), iş (job), günlük (log), korumalı alan (sandbox); "AI" stays "AI" (macOS writes "yapay zekâ" in running text)

| German key | Turkish |
|---|---|
| Ablage | Dosya |
| Bearbeiten | Düzen |
| Darstellung | Görüntü |
| Fenster | Pencere |
| Hilfe | Yardım |
| Dienste | Servisler |
| Über {} | {} Hakkında |
| Einstellungen | Ayarlar (menu item: Ayarlar…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | Gizle: {} / Diğerlerini Gizle / Tümünü Göster |
| {} beenden | Çık: {} |
| Widerrufen / Wiederholen | Geri Al / Yinele |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Kes / Kopyala / Yapıştır / Sil |
| Alles auswählen | Tümünü Seç |
| Suchen … / Weitersuchen / Rückwärts suchen | Bul… / Sonrakini Bul / Öncekini Bul |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Simge Durumuna Küçült / Büyüt/Küçült / Tümünü Öne Getir |
| Vollbildmodus | Tam Ekrana Geç |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Araç Çubuğunu Gizle / Kenar Çubuğunu Göster/Gizle |
| Sichern | Kaydet |
| Öffnen / Schließen | Aç / Kapat |
| Abbrechen / OK | Vazgeç / Tamam |
| Im Finder zeigen | Finder’da Göster |
| Papierkorb | Çöp Sepeti |
| Schlüsselbund | Anahtar Zinciri |
| Zwischenablage | Pano |
| Tab | sekme |
| AI-Tab | AI sekmesi |
| Panel | panel |
| Seitenleiste | kenar çubuğu |
| Vorschau | önizleme |
| Aktivität | Etkinlik |
| Projekt / Ordner | proje / klasör |
| Vertrauen, vertrauen | güven, güvenmek |
| Platte | disk (diskte) |
| Unterschiede | farklar |
| Zusammenführen | birleştirme, birleştir |
| Konflikt | çakışma |
| Treffer | sonuç |
| Suche | Arama |
| Ersetzen | Değiştir |
| Sprachserver | dil sunucusu |
| Hinweis (Schweregrad) | İpucu |
| Lesezeichen / Verlauf / Adresse | yer işareti / geçmiş / adres |
| Bytes | bayt |
| Exportieren | Dışa Aktar |
| Format | Biçim |
| Diktat | dikte |
| Agent | ajan |
| Eigener Agent | yerleşik ajan |
| Eigene Agenten | Özel ajanlar |
| Thread (AI-Thread, CLI-Thread) | sohbet (AI sohbeti, CLI sohbeti) |
| Sitzung | oturum |
| Unterhaltung | konuşma |
| Zug | tur |
| Antwort | yanıt |
| Rückfrage | soru |
| Freigabe, Freigabemodus | izin, izin modu |
| Umgehen | Atla |
| Erlauben / Ablehnen / Immer erlauben … | İzin Ver / Reddet / Her Zaman İzin Ver… |
| Plan | plan |
| To-dos | Yapılacaklar |
| Aufgabe | görev |
| Auftrag | görev |
| Werkzeug | araç |
| Skills | Beceriler |
| Befehl | komut |
| Hook | hook |
| Sandbox | korumalı alan |
| Fremder Inhalt | Dış İçerik |
| Sicherungspunkt | kontrol noktası |
| Zurückspulen | geri sarma, geri sar |
| Verdichten | sıkıştırma, sıkıştır |
| Kontext, Kontextfenster | bağlam, bağlam penceresi |
| Denktiefe | düşünme derinliği |
| Modell / Anbieter / Endpunkt | model / sağlayıcı / uç nokta |
| Token, Tokens | token, token’lar |
| API-Schlüssel | API anahtarı |
| Register | kayıt defteri |
| Adapter | bağdaştırıcı |
| Bereit | Hazır |
| Prüfsumme | sağlama toplamı |
| Anmeldung, anmelden | giriş, giriş yap |
| Konto | hesap |
| Abo | Abonelik |
| Repository | depo |
| Commit, committen | commit, commit et |
| Secrets | gizli bilgiler |
| vormerken | hazırla |
| Zurücknehmen | Hazırlıktan Çıkar |
| nicht versioniert | izlenmiyor |
| Abschnitt (Git) | parça |
| Branch / Remote | dal / uzak depo |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Übernehmen (Worktree) | aktar |
| Arbeitskopie | çalışma kopyası |
| Lauf, Läufe | çalıştırma, çalıştırmalar |
| Host | sunucu |
| Job | iş |
| Log | günlük |

### Ukrainian (`uk`)
- Address: formal "ви", lowercase as macOS Ukrainian writes it ("ви", "ваш"); menu items and buttons in the infinitive
  ("Зберегти", "Сховати бічну панель"); instructions in the "ви" imperative ("Виберіть…", "Відкрийте…")
- Typography: «…»; apostrophe "ʼ" (U+02BC), as macOS writes it ("обʼєднати", "компʼютер", "вʼязка"); menu items end in
  "…" without a space ("Параметри…", "Зберегти як…"); the OK button is "OK" in Latin letters; sentence case
- Git and CI words: Git's operation names stay in Latin as Ukrainian developers write them: push, pull, fetch, rebase,
  worktree; Ukrainian otherwise: репозиторій, коміт (закомітити), гілка (branch), віддалений репозиторій (remote),
  індексувати (stage), фрагмент (hunk), злиття (merge), робоча копія, хук, пісочниця, токен, запуск (CI run), завдання
  (job), журнал (log); "AI" is "ШІ", as macOS writes it

| German key | Ukrainian |
|---|---|
| Ablage | Файл |
| Bearbeiten | Редагування |
| Darstellung | Перегляд |
| Fenster | Вікно |
| Hilfe | Довідка |
| Dienste | Служби |
| Über {} | Про {} |
| Einstellungen | Параметри (menu item: Параметри…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | Сховати {} / Сховати решту / Показати всі |
| {} beenden | Завершити {} |
| Widerrufen / Wiederholen | Відмінити / Повторити |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Вирізати / Скопіювати / Вставити / Видалити |
| Alles auswählen | Вибрати всі |
| Suchen … / Weitersuchen / Rückwärts suchen | Пошук… / Знайти наступне / Знайти попереднє |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Згорнути / Оптимізувати / Усі наперед |
| Vollbildmodus | Увійти до повноекранного режиму |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Сховати панель інструментів / Показати/сховати бічну панель |
| Sichern | Зберегти |
| Öffnen / Schließen | Відкрити / Закрити |
| Abbrechen / OK | Скасувати / OK |
| Im Finder zeigen | Показати у Finder |
| Papierkorb | Смітник |
| Schlüsselbund | вʼязка |
| Zwischenablage | буфер обміну |
| Tab | вкладка |
| AI-Tab | вкладка ШІ |
| Panel | панель |
| Seitenleiste | бічна панель |
| Vorschau | попередній перегляд |
| Aktivität | Активність |
| Projekt / Ordner | проєкт / папка |
| Vertrauen, vertrauen | довіра, довіряти |
| Platte | диск (на диску) |
| Unterschiede | відмінності |
| Zusammenführen | злиття, злити |
| Konflikt | конфлікт |
| Treffer | результат |
| Suche | Пошук |
| Ersetzen | Замінити |
| Sprachserver | мовний сервер |
| Hinweis (Schweregrad) | Підказка |
| Lesezeichen / Verlauf / Adresse | закладка / історія / адреса |
| Bytes | Б |
| Exportieren | Експортувати |
| Format | Формат |
| Diktat | диктування |
| Agent | агент |
| Eigener Agent | власний агент |
| Eigene Agenten | Користувацькі агенти |
| Thread (AI-Thread, CLI-Thread) | чат (ШІ-чат, CLI-чат) |
| Sitzung | сеанс |
| Unterhaltung | діалог |
| Zug | хід |
| Antwort | відповідь |
| Rückfrage | запитання |
| Freigabe, Freigabemodus | дозвіл, режим дозволів |
| Umgehen | Обхід |
| Erlauben / Ablehnen / Immer erlauben … | Дозволити / Заборонити / Завжди дозволяти… |
| Plan | план |
| To-dos | Список справ |
| Aufgabe | задача |
| Auftrag | доручення |
| Werkzeug | інструмент |
| Skills | Навички |
| Befehl | команда |
| Hook | хук |
| Sandbox | пісочниця |
| Fremder Inhalt | Зовнішній вміст |
| Sicherungspunkt | контрольна точка |
| Zurückspulen | відкат, відкотити |
| Verdichten | стиснення, стиснути |
| Kontext, Kontextfenster | контекст, контекстне вікно |
| Denktiefe | глибина міркувань |
| Modell / Anbieter / Endpunkt | модель / провайдер / кінцева точка |
| Token, Tokens | токен, токени |
| API-Schlüssel | ключ API |
| Register | реєстр |
| Adapter | адаптер |
| Bereit | Готово |
| Prüfsumme | контрольна сума |
| Anmeldung, anmelden | вхід, увійти |
| Konto | обліковий запис |
| Abo | Підписка |
| Repository | репозиторій |
| Commit, committen | коміт, закомітити |
| Secrets | секрети |
| vormerken | індексувати |
| Zurücknehmen | Вилучити з індексу |
| nicht versioniert | не відстежується |
| Abschnitt (Git) | фрагмент |
| Branch / Remote | гілка / віддалений репозиторій |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Übernehmen (Worktree) | перенести |
| Arbeitskopie | робоча копія |
| Lauf, Läufe | запуск, запуски |
| Host | хост |
| Job | завдання |
| Log | журнал |

### Vietnamese (`vi`)
- Address: "bạn", as macOS Vietnamese ("Bạn có chắc chắn muốn …?"); menu items and buttons are bare verbs ("Lưu", "Ẩn
  thanh công cụ"), instructions without a pronoun where possible ("Chọn …")
- Typography: “…”; menu items end in "…" without a space ("Cài đặt…", "Lưu thành…"); sentence case, but the capitals
  macOS uses inside standard items stay as they are ("Chuyển sang Toàn Màn hình", "Ẩn Thanh bên")
- Git and CI words: kept as Vietnamese developers say them: commit, stage, remote, push, pull, fetch, rebase, worktree,
  hook, sandbox, token, job; Vietnamese where the word is settled: kho lưu trữ (repository), nhánh (branch), hợp nhất
  (merge), xung đột (conflict), khối thay đổi (hunk), bản sao làm việc (working tree), lần chạy (CI run), nhật ký (log);
  "AI" stays "AI"

| German key | Vietnamese |
|---|---|
| Ablage | Tệp |
| Bearbeiten | Sửa |
| Darstellung | Xem |
| Fenster | Cửa sổ |
| Hilfe | Trợ giúp |
| Dienste | Dịch vụ |
| Über {} | Giới thiệu về {} |
| Einstellungen | Cài đặt (menu item: Cài đặt…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | Ẩn {} / Ẩn các mục khác / Hiển thị tất cả |
| {} beenden | Thoát {} |
| Widerrufen / Wiederholen | Hoàn tác / Làm lại |
| Ausschneiden / Kopieren / Einsetzen / Löschen | Cắt / Sao chép / Dán / Xóa |
| Alles auswählen | Chọn Tất cả |
| Suchen … / Weitersuchen / Rückwärts suchen | Tìm… / Tìm mục tiếp theo / Tìm mục trước |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | Thu nhỏ / Thu phóng / Đưa tất cả lên Phía trước |
| Vollbildmodus | Chuyển sang Toàn Màn hình |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | Ẩn thanh công cụ / Hiển thị/Ẩn Thanh bên |
| Sichern | Lưu |
| Öffnen / Schließen | Mở / Đóng |
| Abbrechen / OK | Hủy / OK |
| Im Finder zeigen | Hiển thị trong Finder |
| Papierkorb | Thùng rác |
| Schlüsselbund | Chuỗi khóa |
| Zwischenablage | bảng nhớ tạm |
| Tab | tab |
| AI-Tab | tab AI |
| Panel | khung |
| Seitenleiste | thanh bên |
| Vorschau | xem trước |
| Aktivität | Hoạt động |
| Projekt / Ordner | dự án / thư mục |
| Vertrauen, vertrauen | sự tin cậy, tin cậy |
| Platte | ổ đĩa (trên ổ đĩa) |
| Unterschiede | khác biệt |
| Zusammenführen | hợp nhất |
| Konflikt | xung đột |
| Treffer | kết quả |
| Suche | Tìm kiếm |
| Ersetzen | Thay thế |
| Sprachserver | máy chủ ngôn ngữ |
| Hinweis (Schweregrad) | Gợi ý |
| Lesezeichen / Verlauf / Adresse | dấu trang / lịch sử / địa chỉ |
| Bytes | byte |
| Exportieren | Xuất |
| Format | Định dạng |
| Diktat | đọc chính tả |
| Agent | tác nhân |
| Eigener Agent | tác nhân tích hợp |
| Eigene Agenten | Tác nhân tùy chỉnh |
| Thread (AI-Thread, CLI-Thread) | luồng (luồng AI, luồng CLI) |
| Sitzung | phiên |
| Unterhaltung | cuộc hội thoại |
| Zug | lượt |
| Antwort | câu trả lời |
| Rückfrage | câu hỏi |
| Freigabe, Freigabemodus | quyền, chế độ quyền |
| Umgehen | Bỏ qua quyền |
| Erlauben / Ablehnen / Immer erlauben … | Cho phép / Từ chối / Luôn cho phép… |
| Plan | kế hoạch |
| To-dos | Việc cần làm |
| Aufgabe | tác vụ |
| Auftrag | nhiệm vụ |
| Werkzeug | công cụ |
| Skills | Skill |
| Befehl | lệnh |
| Hook | hook |
| Sandbox | sandbox |
| Fremder Inhalt | Nội dung bên ngoài |
| Sicherungspunkt | điểm kiểm tra |
| Zurückspulen | tua lại |
| Verdichten | thu gọn |
| Kontext, Kontextfenster | ngữ cảnh, cửa sổ ngữ cảnh |
| Denktiefe | mức độ suy nghĩ |
| Modell / Anbieter / Endpunkt | mô hình / nhà cung cấp / điểm cuối |
| Token, Tokens | token |
| API-Schlüssel | khóa API |
| Register | sổ đăng ký |
| Adapter | bộ chuyển đổi |
| Bereit | Sẵn sàng |
| Prüfsumme | tổng kiểm tra |
| Anmeldung, anmelden | đăng nhập |
| Konto | tài khoản |
| Abo | Gói đăng ký |
| Repository | kho lưu trữ |
| Commit, committen | commit |
| Secrets | thông tin bí mật |
| vormerken | stage |
| Zurücknehmen | Bỏ stage |
| nicht versioniert | chưa theo dõi |
| Abschnitt (Git) | khối thay đổi |
| Branch / Remote | nhánh / remote |
| Push / Pull / Fetch / Rebase | push / pull / fetch / rebase |
| Worktree | worktree |
| Übernehmen (Worktree) | tiếp nhận |
| Arbeitskopie | bản sao làm việc |
| Lauf, Läufe | lần chạy |
| Host | máy chủ |
| Job | job |
| Log | nhật ký |

### Chinese, Simplified (`zh-Hans`)
- Address: "你", as macOS Simplified Chinese (never "您"); menu items and buttons are bare verbs ("保存", "隐藏工具栏")
- Typography: “…” and full-width punctuation (，。：？！（）); menu items end in "…" without a space ("设置…", "保存为…"); a space
  between Chinese and Latin text or a placeholder, as splat writes it ("关于 {}", "AI 标签页"), although macOS's own strings
  leave it out
- Git and CI words: Chinese, as Chinese developers and Git's Chinese documentation use them: 仓库, 提交, 暂存, 分支, 远程仓库, 推送 /
  拉取 / 抓取 / 变基, 合并, 冲突, 变更块 (hunk), 工作树 (worktree), 工作区 (working tree), 钩子 (hook), 运行 (CI run), 作业 (job), 日志 (log);
  "Token" and "AI" stay Latin

| German key | Chinese (Simplified) |
|---|---|
| Ablage | 文件 |
| Bearbeiten | 编辑 |
| Darstellung | 显示 |
| Fenster | 窗口 |
| Hilfe | 帮助 |
| Dienste | 服务 |
| Über {} | 关于 {} |
| Einstellungen | 设置 (menu item: 设置…) |
| {} ausblenden / Andere ausblenden / Alle einblenden | 隐藏 {} / 隐藏其他 / 全部显示 |
| {} beenden | 退出 {} |
| Widerrufen / Wiederholen | 撤销 / 重做 |
| Ausschneiden / Kopieren / Einsetzen / Löschen | 剪切 / 拷贝 / 粘贴 / 删除 |
| Alles auswählen | 全选 |
| Suchen … / Weitersuchen / Rückwärts suchen | 查找… / 查找下一个 / 查找上一个 |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | 最小化 / 缩放 / 前置全部窗口 |
| Vollbildmodus | 进入全屏幕 |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | 隐藏工具栏 / 显示/隐藏边栏 |
| Sichern | 保存 |
| Öffnen / Schließen | 打开 / 关闭 |
| Abbrechen / OK | 取消 / 好 |
| Im Finder zeigen | 在访达中显示 |
| Papierkorb | 废纸篓 |
| Schlüsselbund | 钥匙串 |
| Zwischenablage | 剪贴板 |
| Tab | 标签页 |
| AI-Tab | AI 标签页 |
| Panel | 面板 |
| Seitenleiste | 边栏 |
| Vorschau | 预览 |
| Aktivität | 活动 |
| Projekt / Ordner | 项目 / 文件夹 |
| Vertrauen, vertrauen | 信任 |
| Platte | 磁盘 (磁盘上) |
| Unterschiede | 差异 |
| Zusammenführen | 合并 |
| Konflikt | 冲突 |
| Treffer | 结果 |
| Suche | 搜索 |
| Ersetzen | 替换 |
| Sprachserver | 语言服务器 |
| Hinweis (Schweregrad) | 提示 |
| Lesezeichen / Verlauf / Adresse | 书签 / 历史记录 / 地址 |
| Bytes | 字节 |
| Exportieren | 导出 |
| Format | 格式 |
| Diktat | 听写 |
| Agent | 智能体 |
| Eigener Agent | 内置智能体 |
| Eigene Agenten | 自定义智能体 |
| Thread (AI-Thread, CLI-Thread) | 对话 (AI 对话, CLI 对话) |
| Sitzung | 会话 |
| Unterhaltung | 对话内容 |
| Zug | 轮 |
| Antwort | 回答 |
| Rückfrage | 提问 |
| Freigabe, Freigabemodus | 权限, 权限模式 |
| Umgehen | 绕过 |
| Erlauben / Ablehnen / Immer erlauben … | 允许 / 拒绝 / 始终允许… |
| Plan | 计划 |
| To-dos | 待办事项 |
| Aufgabe | 任务 |
| Auftrag | 任务 |
| Werkzeug | 工具 |
| Skills | 技能 |
| Befehl | 命令 |
| Hook | 钩子 |
| Sandbox | 沙盒 |
| Fremder Inhalt | 外部内容 |
| Sicherungspunkt | 检查点 |
| Zurückspulen | 回退 |
| Verdichten | 压缩 |
| Kontext, Kontextfenster | 上下文, 上下文窗口 |
| Denktiefe | 思考深度 |
| Modell / Anbieter / Endpunkt | 模型 / 提供商 / 端点 |
| Token, Tokens | Token |
| API-Schlüssel | API 密钥 |
| Register | 注册表 |
| Adapter | 适配器 |
| Bereit | 就绪 |
| Prüfsumme | 校验和 |
| Anmeldung, anmelden | 登录 |
| Konto | 账户 |
| Abo | 订阅 |
| Repository | 仓库 |
| Commit, committen | 提交 |
| Secrets | 机密信息 |
| vormerken | 暂存 |
| Zurücknehmen | 取消暂存 |
| nicht versioniert | 未跟踪 |
| Abschnitt (Git) | 变更块 |
| Branch / Remote | 分支 / 远程仓库 |
| Push / Pull / Fetch / Rebase | 推送 / 拉取 / 抓取 / 变基 |
| Worktree | 工作树 |
| Übernehmen (Worktree) | 并入 |
| Arbeitskopie | 工作区 |
| Lauf, Läufe | 运行 |
| Host | 主机 |
| Job | 作业 |
| Log | 日志 |

### Chinese, Traditional (`zh-Hant`)
- Address: "你", as macOS Traditional Chinese (never "您"); Taiwan usage and macOS's Taiwan words (檔案, 視窗, 設定, 打開); menu
  items and buttons are bare verbs ("儲存", "隱藏工具列")
- Typography: 「…」 (『…』 inside) and full-width punctuation (，。：？！（）); menu items end in "⋯" (U+22EF) without a space, as
  macOS writes it ("設定⋯", "儲存為⋯"); a space between Chinese and Latin text or a placeholder, as splat writes it ("關於 {}",
  "AI 標籤頁"), although macOS's own strings leave it out
- Git and CI words: Chinese, as Taiwanese developers and the Traditional Chinese Git tools use them: 儲存庫, 提交, 暫存, 分支,
  遠端, 推送 / 提取 / 擷取 / 重訂基底, 合併, 衝突, 變更區塊 (hunk), 工作樹 (worktree), 工作目錄 (working tree), 掛鉤 (hook), 執行 (CI run), 作業 (job),
  記錄 (log); "Token" and "AI" stay Latin

| German key | Chinese (Traditional) |
|---|---|
| Ablage | 檔案 |
| Bearbeiten | 編輯 |
| Darstellung | 顯示方式 |
| Fenster | 視窗 |
| Hilfe | 輔助說明 |
| Dienste | 服務 |
| Über {} | 關於 {} |
| Einstellungen | 設定 (menu item: 設定⋯) |
| {} ausblenden / Andere ausblenden / Alle einblenden | 隱藏 {} / 隱藏其他 / 顯示全部 |
| {} beenden | 結束 {} |
| Widerrufen / Wiederholen | 還原 / 重做 |
| Ausschneiden / Kopieren / Einsetzen / Löschen | 剪下 / 拷貝 / 貼上 / 刪除 |
| Alles auswählen | 全選 |
| Suchen … / Weitersuchen / Rückwärts suchen | 尋找⋯ / 尋找下一個 / 尋找上一個 |
| Im Dock ablegen / Zoomen / Alle nach vorne bringen | 縮到最小 / 縮放 / 將此程式所有視窗移至最前 |
| Vollbildmodus | 進入全螢幕 |
| Symbolleiste ausblenden / Seitenleiste ein-/ausblenden | 隱藏工具列 / 顯示／隱藏側邊欄 |
| Sichern | 儲存 |
| Öffnen / Schließen | 打開 / 關閉 |
| Abbrechen / OK | 取消 / 好 |
| Im Finder zeigen | 顯示於 Finder |
| Papierkorb | 垃圾桶 |
| Schlüsselbund | 鑰匙圈 |
| Zwischenablage | 剪貼板 |
| Tab | 標籤頁 |
| AI-Tab | AI 標籤頁 |
| Panel | 面板 |
| Seitenleiste | 側邊欄 |
| Vorschau | 預覽 |
| Aktivität | 活動 |
| Projekt / Ordner | 專案 / 檔案夾 |
| Vertrauen, vertrauen | 信任 |
| Platte | 磁碟 (磁碟上) |
| Unterschiede | 差異 |
| Zusammenführen | 合併 |
| Konflikt | 衝突 |
| Treffer | 結果 |
| Suche | 搜尋 |
| Ersetzen | 取代 |
| Sprachserver | 語言伺服器 |
| Hinweis (Schweregrad) | 提示 |
| Lesezeichen / Verlauf / Adresse | 書籤 / 瀏覽記錄 / 網址 |
| Bytes | 位元組 |
| Exportieren | 輸出 |
| Format | 格式 |
| Diktat | 聽寫 |
| Agent | 代理程式 |
| Eigener Agent | 內建代理程式 |
| Eigene Agenten | 自訂代理程式 |
| Thread (AI-Thread, CLI-Thread) | 對話 (AI 對話, CLI 對話) |
| Sitzung | 工作階段 |
| Unterhaltung | 對話內容 |
| Zug | 輪 |
| Antwort | 回答 |
| Rückfrage | 提問 |
| Freigabe, Freigabemodus | 權限, 權限模式 |
| Umgehen | 略過權限 |
| Erlauben / Ablehnen / Immer erlauben … | 允許 / 拒絕 / 永遠允許⋯ |
| Plan | 計畫 |
| To-dos | 待辦事項 |
| Aufgabe | 任務 |
| Auftrag | 指派內容 |
| Werkzeug | 工具 |
| Skills | 技能 |
| Befehl | 指令 |
| Hook | 掛鉤 |
| Sandbox | 沙盒 |
| Fremder Inhalt | 外部內容 |
| Sicherungspunkt | 檢查點 |
| Zurückspulen | 回溯 |
| Verdichten | 壓縮 |
| Kontext, Kontextfenster | 上下文, 上下文視窗 |
| Denktiefe | 思考深度 |
| Modell / Anbieter / Endpunkt | 模型 / 供應商 / 端點 |
| Token, Tokens | Token |
| API-Schlüssel | API 金鑰 |
| Register | 登錄庫 |
| Adapter | 轉接器 |
| Bereit | 就緒 |
| Prüfsumme | 核對碼 |
| Anmeldung, anmelden | 登入 |
| Konto | 帳號 |
| Abo | 訂閱 |
| Repository | 儲存庫 |
| Commit, committen | 提交 |
| Secrets | 機密資訊 |
| vormerken | 暫存 |
| Zurücknehmen | 取消暫存 |
| nicht versioniert | 未追蹤 |
| Abschnitt (Git) | 變更區塊 |
| Branch / Remote | 分支 / 遠端 |
| Push / Pull / Fetch / Rebase | 推送 / 提取 / 擷取 / 重訂基底 |
| Worktree | 工作樹 |
| Übernehmen (Worktree) | 整合 |
| Arbeitskopie | 工作目錄 |
| Lauf, Läufe | 執行 |
| Host | 主機 |
| Job | 作業 |
| Log | 記錄 |
