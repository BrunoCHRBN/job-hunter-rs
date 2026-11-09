#!/usr/bin/env python3
"""
Bridge script para coleta de vagas utilizando python-jobspy.
Imprime exclusivamente um array JSON no stdout com os campos:
external_id, source, title, company, location, url, raw_description, published_at.
"""

import argparse
import json
import sys
from typing import Any, Dict, List


def safe_str(val: Any) -> str:
    if val is None:
        return ""
    s = str(val).strip()
    if s.lower() in ("nan", "none", "null", "<na>"):
        return ""
    return s


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
        sys.stderr.write(f" jobspy indisponível: {exc}\n")
        print("[]")
        return 0

    try:
        df = scrape_jobs(
            site_name=["linkedin", "indeed", "glassdoor"],
            search_term=args.query,
            location=args.location,
            results_wanted=args.results,
            hours_old=args.hours_old,
            is_remote=args.remote,
            country_indeed="Brazil",
        )
    except Exception as exc:
        sys.stderr.write(f"Erro ao executar scrape_jobs: {exc}\n")
        print("[]")
        return 0

    if df is None or len(df) == 0:
        print("[]")
        return 0

    records: List[Dict[str, Any]] = []
    for idx, row in df.iterrows():
        raw_id = safe_str(row.get("id"))
        site = safe_str(row.get("site")) or "jobspy"
        url = safe_str(row.get("job_url"))
        title = safe_str(row.get("title"))
        company = safe_str(row.get("company"))
        location = safe_str(row.get("location"))
        description = safe_str(row.get("description"))
        date_posted = safe_str(row.get("date_posted"))

        if not title and not url:
            continue

        external_id = raw_id if raw_id else f"{site}-{idx}"
        records.append(
            {
                "external_id": external_id,
                "source": f"jobspy:{site}",
                "title": title or "Sem Título",
                "company": company or "Empresa Confidencial",
                "location": location or args.location,
                "url": url,
                "raw_description": description,
                "published_at": date_posted if date_posted else None,
            }
        )

    print(json.dumps(records, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
