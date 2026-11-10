#!/usr/bin/env python3
"""
Bridge script para coleta de vagas utilizando python-jobspy.
Imprime exclusivamente um array JSON no stdout com os campos:
external_id, source, title, company, location, url, raw_description, published_at.
Envia logs estruturados de status por rede para o stderr (SITE_STATS / SITE_ERR).
"""

import argparse
import json
import sys
from typing import Any, Dict, List, Set


def safe_str(val: Any) -> str:
    if val is None:
        return ""
    s = str(val).strip()
    if s.lower() in ("nan", "none", "null", "<na>"):
        return ""
    return s


def parse_locations(raw_loc: str) -> List[str]:
    cleaned = raw_loc.strip()
    if not cleaned or cleaned.lower() in ("brasil", "brazil", "remoto"):
        return ["Brasil"]

    # Se houver separador de vírgula ou ponto-e-vírgula, separa em cidades individuais
    parts = [p.strip() for p in cleaned.replace(";", ",").split(",") if p.strip()]
    if not parts:
        return ["Brasil"]
    # Limita a no máximo 3 cidades por rodada para não estourar o tempo de resposta
    return parts[:3]


def main() -> int:
    parser = argparse.ArgumentParser(description="JobSpy Bridge para Job Hunter RS")
    parser.add_argument("--query", required=True, help="Termo de busca")
    parser.add_argument("--location", default="Brasil", help="Localização da vaga")
    parser.add_argument("--results", type=int, default=15, help="Número de resultados por site")
    parser.add_argument("--hours-old", type=int, default=168, help="Idade máxima das vagas em horas")
    parser.add_argument("--remote", action="store_true", help="Filtrar apenas vagas remotas")
    args = parser.parse_args()

    try:
        from jobspy import scrape_jobs  # type: ignore
    except Exception as exc:
        sys.stderr.write(
            f"JOB_SPY_ERROR: Pacote 'jobspy' não está instalado no interpretador atual ({sys.executable}). "
            f"Instale as dependências com: pip install -r requirements.txt. Detalhe: {exc}\n"
        )
        print("[]")
        return 2

    locations = parse_locations(args.location)
    sites = ["linkedin", "indeed", "glassdoor"]
    per_site_results = max(3, args.results // len(locations)) if len(locations) > 1 else args.results

    records: List[Dict[str, Any]] = []
    seen_keys: Set[str] = set()

    for site in sites:
        site_total = 0
        for loc in locations:
            try:
                df = scrape_jobs(
                    site_name=[site],
                    search_term=args.query,
                    location=loc,
                    results_wanted=per_site_results,
                    hours_old=args.hours_old,
                    is_remote=args.remote,
                    country_indeed="Brazil",
                )
            except Exception as exc:
                sys.stderr.write(f"SITE_ERR:{site}:{loc}={exc}\n")
                continue

            if df is None or len(df) == 0:
                continue

            for idx, row in df.iterrows():
                raw_id = safe_str(row.get("id"))
                site_name = safe_str(row.get("site")) or site
                url = safe_str(row.get("job_url"))
                title = safe_str(row.get("title"))
                company = safe_str(row.get("company"))
                row_loc = safe_str(row.get("location"))
                description = safe_str(row.get("description"))
                date_posted = safe_str(row.get("date_posted"))

                if not title and not url:
                    continue

                external_id = raw_id if raw_id else f"{site_name}-{idx}"
                dedup_key = f"{site_name}:{url or external_id}"
                if dedup_key in seen_keys:
                    continue
                seen_keys.add(dedup_key)

                records.append(
                    {
                        "external_id": external_id,
                        "source": f"jobspy:{site_name}",
                        "title": title or "Sem Título",
                        "company": company or "Empresa Confidencial",
                        "location": row_loc or loc,
                        "url": url,
                        "raw_description": description,
                        "published_at": date_posted if date_posted else None,
                    }
                )
                site_total += 1

        sys.stderr.write(f"SITE_STATS:{site}={site_total}\n")

    print(json.dumps(records, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
