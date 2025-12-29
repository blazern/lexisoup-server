BEGIN IMMEDIATE;

ALTER TABLE expr ADD COLUMN denotation_count INTEGER NOT NULL DEFAULT 0;

WITH counts AS (
  SELECT expr AS expr_id, COUNT(*) AS cnt
  FROM denotationx
  GROUP BY expr
)
UPDATE expr
SET denotation_count = COALESCE(
  (SELECT cnt FROM counts WHERE counts.expr_id = expr.id),
  0
);

CREATE VIRTUAL TABLE IF NOT EXISTS spell USING spellfix1;

INSERT INTO spell(word, rank, langid)
SELECT
  e.txt AS word,
  e.denotation_count AS rank,
  e.langvar AS langid
FROM expr e
WHERE 5 <= denotation_count;

COMMIT;
