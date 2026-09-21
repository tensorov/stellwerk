# Context budget — измерено, не гипотеза

Метод: промпт "." на модели neuraldeep/qwen3.6-unlim-noreason,
счётчик input провайдера. Дата: <впиши>.

| # | Конфиг                    | Токены | Дельта       |
|---|---------------------------|-------:|-------------:|
| A | clean OpenCode            |  8 602 | базовый      |
| B | A + polyforge MCP         |  9 415 | +813         |
| C | A + OMO                   | 56 224 | +47 622      |
| D | C + polyforge             | 57 001 | +777         |
| E | D + all MCP (lsp; grep_app недоступен) | 58 038 | +1 037 |

## Решения
- Worker: конфиг B. Лимит qwen 46 112, свободно 36 697.
- OMO: недопустим для любой модели <64k окна. Только planner ≥128k.
- polyforge: autoload в worker (813 токенов).
- lsp: разрешён по требованию (~1k, замерено в E).
- grep_app: DEFERRED — стоимость не измерена (сеть недоступна на момент замера).
- Связь planner↔worker: только через .sisyphus/plans/<id>/packet-NN.md.

## Инварианты packet (следуют из бюджета)
- ≤3 файла, ≤200 строк diff
- вывод тестов через output shaping >2k
- packet ≤4k токенов
