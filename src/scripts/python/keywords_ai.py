import spacy

# Загружаем английскую модель
nlp = spacy.load("en_core_web_sm")

def extract_keywords(texts: list[str]) -> list[list[str]]:
    result = []
    for t in texts:
        if t == "":
            continue
        doc = nlp(t)
        # Берём только имена собственные (PERSON, ORG, WORK_OF_ART и т.п.)
        # keywords = [ent.text for ent in doc.ents if ent.label_ in ("PERSON", "ORG", "WORK_OF_ART")]
        keywords = []

        for ent in doc.ents:
            if ent.label_ in ("PERSON", "ORG"):
                if ent.text.lower().strip().startswith("•"):
                    continue

                if ent.text.lower() not in ["tiktok", "youtube", "shorts", "facebook", "netflix", "instagram", "shorts - youtube", "♡"]:
                    keywords.append(ent.text)

        result.append(keywords)
    return result