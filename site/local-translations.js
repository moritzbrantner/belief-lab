export const translations = {
  en: {
    local: 'Local models', language: 'Language', theme: 'Theme', system: 'System', light: 'Light', dark: 'Dark',
    eyebrow: 'EVIDENCE → MODEL → BELIEF', title: 'Ask the model.',
    intro: 'Load a bounded evidence request and inspect the model’s judgment, the resulting belief, and their provenance.',
    private: 'Runs on your computer', requestTitle: '1. Prepare a request', load: 'Load example', file: 'Request file', model: 'Model',
    fixture: 'Scripted provider · no model', requestHelp: 'The request includes the bounded source text, question, proposition, evidence references, and explicit policy. Keep references consistent when editing your source content.',
    requestJson: 'Decision request JSON', downloadHelp: 'The first run downloads the selected model and prepares SemIf. Later runs reuse local resources. Scoring sends no evidence to a remote model service.',
    run: 'Run analysis', download: 'Download result', resultTitle: '2. Inspect the result', full: 'Full result and provenance',
    footer: 'Model option scores are conditional probabilities. Belief soft truth is a heuristic, not a calibrated probability.',
    ready: 'Ready to analyze. No model has been started.', loading: 'Loading request…', running: 'Preparing the model and analyzing… First setup can take several minutes; progress appears in the terminal.',
    complete: 'Analysis complete.', disconnected: 'Open the full session URL printed by cargo run -- workbench.',
    tooLarge: 'Requests must be at most 1 MiB.', invalid: 'Expected a belief_semantic_request@1 request with a supported provider.',
    failed: 'Request failed', unknown: 'The judgment is unknown. No belief was derived.', judgment: 'Judgment', belief: 'Belief',
    option: 'Option', probability: 'Conditional option probability', source: 'Source', revision: 'Revision', producer: 'Producer', simulated: 'Scripted result — no model ran.'
  },
  de: {
    local: 'Lokale Modelle', language: 'Sprache', theme: 'Darstellung', system: 'System', light: 'Hell', dark: 'Dunkel',
    eyebrow: 'EVIDENZ → MODELL → ÜBERZEUGUNG', title: 'Das Modell befragen.',
    intro: 'Lade eine begrenzte Evidenzanfrage und prüfe Modellurteil, abgeleitete Überzeugung und Herkunft.',
    private: 'Läuft auf deinem Computer', requestTitle: '1. Anfrage vorbereiten', load: 'Beispiel laden', file: 'Anfragedatei', model: 'Modell',
    fixture: 'Vorgegebene Werte · kein Modell', requestHelp: 'Die Anfrage enthält Quelltext, Frage, Proposition, Evidenzreferenzen und explizite Richtlinien. Halte die Referenzen beim Bearbeiten des Inhalts konsistent.',
    requestJson: 'Entscheidungsanfrage als JSON', downloadHelp: 'Der erste Lauf lädt das gewählte Modell und richtet SemIf ein. Spätere Läufe verwenden lokale Ressourcen. Die Bewertung sendet keine Evidenz an einen entfernten Modelldienst.',
    run: 'Analyse starten', download: 'Ergebnis herunterladen', resultTitle: '2. Ergebnis prüfen', full: 'Vollständiges Ergebnis und Herkunft',
    footer: 'Modellwerte sind bedingte Optionswahrscheinlichkeiten. Soft Truth ist eine Heuristik, keine kalibrierte Wahrscheinlichkeit.',
    ready: 'Bereit zur Analyse. Noch kein Modell gestartet.', loading: 'Anfrage wird geladen…', running: 'Modell wird vorbereitet und analysiert… Die erste Einrichtung kann einige Minuten dauern; Fortschritt erscheint im Terminal.',
    complete: 'Analyse abgeschlossen.', disconnected: 'Öffne die vollständige Sitzungs-URL aus cargo run -- workbench.',
    tooLarge: 'Anfragen dürfen höchstens 1 MiB groß sein.', invalid: 'Eine belief_semantic_request@1-Anfrage mit unterstütztem Anbieter wird erwartet.',
    failed: 'Anfrage fehlgeschlagen', unknown: 'Das Urteil ist unbekannt. Keine Überzeugung wurde abgeleitet.', judgment: 'Urteil', belief: 'Überzeugung',
    option: 'Option', probability: 'Bedingte Optionswahrscheinlichkeit', source: 'Quelle', revision: 'Revision', producer: 'Erzeuger', simulated: 'Vorgegebenes Ergebnis — kein Modell ausgeführt.'
  },
  es: {
    local: 'Modelos locales', language: 'Idioma', theme: 'Tema', system: 'Sistema', light: 'Claro', dark: 'Oscuro',
    eyebrow: 'EVIDENCIA → MODELO → CREENCIA', title: 'Pregunta al modelo.',
    intro: 'Carga una solicitud de evidencia acotada e inspecciona el juicio del modelo, la creencia derivada y su procedencia.',
    private: 'Se ejecuta en tu ordenador', requestTitle: '1. Prepara una solicitud', load: 'Cargar ejemplo', file: 'Archivo de solicitud', model: 'Modelo',
    fixture: 'Valores predeterminados · sin modelo', requestHelp: 'La solicitud incluye el texto fuente, pregunta, proposición, referencias de evidencia y política explícita. Mantén las referencias coherentes al editar el contenido.',
    requestJson: 'Solicitud de decisión JSON', downloadHelp: 'La primera ejecución descarga el modelo seleccionado y prepara SemIf. Las siguientes reutilizan recursos locales. La evaluación no envía evidencia a un servicio de modelos remoto.',
    run: 'Ejecutar análisis', download: 'Descargar resultado', resultTitle: '2. Inspecciona el resultado', full: 'Resultado completo y procedencia',
    footer: 'Las puntuaciones del modelo son probabilidades condicionales de las opciones. Soft Truth es una heurística, no una probabilidad calibrada.',
    ready: 'Listo para analizar. No se ha iniciado ningún modelo.', loading: 'Cargando solicitud…', running: 'Preparando el modelo y analizando… La primera instalación puede tardar varios minutos; el progreso aparece en el terminal.',
    complete: 'Análisis completo.', disconnected: 'Abre la URL de sesión completa que muestra cargo run -- workbench.',
    tooLarge: 'Las solicitudes no deben superar 1 MiB.', invalid: 'Se esperaba belief_semantic_request@1 con un proveedor compatible.',
    failed: 'Solicitud fallida', unknown: 'El juicio es desconocido. No se derivó ninguna creencia.', judgment: 'Juicio', belief: 'Creencia',
    option: 'Opción', probability: 'Probabilidad condicional de la opción', source: 'Fuente', revision: 'Revisión', producer: 'Productor', simulated: 'Resultado predeterminado — no se ejecutó ningún modelo.'
  }
};
