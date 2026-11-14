// Made by Nordan
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Text {
    pub saved: &'static str,
    pub failures: &'static str,
    pub unsupported: &'static str,
    pub no_files: &'static str,
}

impl Text {
    pub fn for_tag(tag: &str) -> Self {
        let (saved, failures, unsupported, no_files) = match tag {
            "pl" => (
                "Zapisano czyste kopie: {}. Pierwsza: {}",
                "Niepowodzenia: {}. Pierwszy błąd: {}",
                "nieobsługiwany format",
                "Wybierz co najmniej jeden plik.",
            ),
            "de" => (
                "Saubere Kopien gespeichert: {}. Erste: {}",
                "Fehlgeschlagen: {}. Erster Fehler: {}",
                "Format nicht unterstützt",
                "Wählen Sie mindestens eine Datei.",
            ),
            "fr" => (
                "Copies propres enregistrées : {}. Première : {}",
                "Échecs : {}. Première erreur : {}",
                "format non pris en charge",
                "Choisissez au moins un fichier.",
            ),
            "es" => (
                "Copias limpias guardadas: {}. Primera: {}",
                "Errores: {}. Primer error: {}",
                "formato no compatible",
                "Elige al menos un archivo.",
            ),
            "it" => (
                "Copie pulite salvate: {}. Prima: {}",
                "Non riusciti: {}. Primo errore: {}",
                "formato non supportato",
                "Scegli almeno un file.",
            ),
            "pt-BR" => (
                "Cópias limpas salvas: {}. Primeira: {}",
                "Falhas: {}. Primeiro erro: {}",
                "formato não suportado",
                "Escolha pelo menos um arquivo.",
            ),
            "uk" => (
                "Збережено чистих копій: {}. Перша: {}",
                "Помилок: {}. Перша помилка: {}",
                "непідтримуваний формат",
                "Виберіть щонайменше один файл.",
            ),
            _ => (
                "Clean copies saved: {}. First: {}",
                "Failed: {}. First error: {}",
                "unsupported format",
                "Choose at least one file.",
            ),
        };
        Self {
            saved,
            failures,
            unsupported,
            no_files,
        }
    }
}

pub fn sentence(template: &str, count: usize, first: &str) -> String {
    // Split first so a file name containing braces stays unchanged.
    let mut parts = template.splitn(3, "{}");
    format!(
        "{}{}{}{}{}",
        parts.next().unwrap_or_default(),
        count,
        parts.next().unwrap_or_default(),
        first,
        parts.next().unwrap_or_default()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_fallback_and_all_supported_tags() {
        let english = Text::for_tag("en");
        for tag in ["", "unknown", "pt", "PL", "en-US"] {
            assert_eq!(Text::for_tag(tag), english);
        }
        for tag in ["pl", "de", "fr", "es", "it", "pt-BR", "uk"] {
            assert_ne!(Text::for_tag(tag), english);
        }
    }
}
