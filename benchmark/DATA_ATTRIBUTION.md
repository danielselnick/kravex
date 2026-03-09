# Benchmark Corpus Attribution

Sample corpus files used for benchmarking and demos. Each dataset has its own license — details below.

## Datasets

### noaa.json — NOAA Weather Station Data

- **Source**: [NOAA Global Historical Climatology Network](https://www.ncei.noaa.gov/products/land-based-station/global-historical-climatology-network-daily)
- **License**: Public domain (U.S. Government work)
- **Content**: Weather observations — temperature, precipitation, snow depth, station metadata with geolocation
- **Full corpus**: ~33.6M documents
- **Obtained via**: [elastic/rally-tracks](https://github.com/elastic/rally-tracks) (noaa track)

### geo.json — Geonames Geographic Features

- **Source**: [GeoNames](https://www.geonames.org/)
- **License**: [Creative Commons Attribution 3.0](https://creativecommons.org/licenses/by/3.0/)
- **Attribution**: GeoNames (geonames.org)
- **Content**: Geographic points of interest — names, coordinates, population, timezone, administrative codes
- **Full corpus**: ~11.4M documents
- **Obtained via**: [elastic/rally-tracks](https://github.com/elastic/rally-tracks) (geonames track)

### pmc.json — PubMed Central Articles

- **Source**: [NCBI PubMed Central](https://www.ncbi.nlm.nih.gov/pmc/)
- **License**: [Creative Commons Attribution 2.0](https://creativecommons.org/licenses/by/2.0/)
- **Attribution**: National Center for Biotechnology Information, U.S. National Library of Medicine
- **Content**: Full-text biomedical and life sciences journal articles
- **Full corpus**: ~574K documents
- **Obtained via**: [elastic/rally-tracks](https://github.com/elastic/rally-tracks) (pmc track)

## Notes

- Sample files in the repository root contain 20-line excerpts in NDJSON format
- Full corpora are downloaded by benchmark scripts at runtime
- Rally-tracks code is licensed under Apache 2.0; the corpus data licenses are as listed above
