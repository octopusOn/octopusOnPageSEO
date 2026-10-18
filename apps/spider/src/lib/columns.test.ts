import { expect, it } from "vitest";
import { columnMatches, columnSelection, moveColumn, parseColumnPreferences, saveColumnOrder, visibleColumns } from "./columns";

const schema=["address","status_code","title","word_count","meta_description"];
const defaults=["address","status_code","title"];

it("searches Turkish labels and ASCII field names with the appropriate case rules",()=>{
  expect(columnMatches("title","Sayfa başlıkları"," TITLE ","tr")).toBe(true);
  expect(columnMatches("lang","Dil","DİL","tr")).toBe(true);
  expect(columnMatches("word_count","Kelime","kelime","tr")).toBe(true);
  expect(columnMatches("title","Sayfa başlıkları","meta","tr")).toBe(false);
});

it("loads independent tab layouts while rejecting malformed or oversized stored values",()=>{
  expect(parseColumnPreferences({internal:["word_count","title","title"],titles:[],unknown:["title"]},["internal","titles"])).toEqual({internal:["word_count","title"],titles:[]});
  for(const value of [null,[],"bad",{internal:"title"},{internal:[1]},{internal:["x".repeat(129)]},{internal:Array(257).fill("title")}]) expect(parseColumnPreferences(value,["internal"])).toEqual({});
});

it("preserves custom order and mandatory URL identity without changing the saved layout",()=>{
  const saved=["word_count","title","removed"];
  expect(visibleColumns(schema,defaults,saved,["address"])).toEqual(["address","word_count","title"]);
  expect(saved).toEqual(["word_count","title","removed"]);
  expect(columnSelection(schema,defaults,["removed"])).toEqual(defaults);
  expect(visibleColumns(schema,defaults,[],["address"])).toEqual(["address"]);
});

it("restores defaults for a reset and ignores missing schema fields without fabricating columns",()=>{
  expect(visibleColumns(schema,defaults,undefined,["address"])).toEqual(defaults);
  expect(visibleColumns(["page","lang"],["page","target","lang"],undefined,["page","target"])).toEqual(["page","lang"]);
  expect(visibleColumns([],defaults,undefined,["address"])).toEqual([]);
});

it("reorders optional fields while protecting fixed endpoints and boundaries",()=>{
  const columns=["source","destination","text","nofollow"];
  const fixed=["source","destination"];
  expect(moveColumn(columns,"nofollow",-1,fixed)).toEqual(["source","destination","nofollow","text"]);
  expect(moveColumn(columns,"text",-1,fixed)).toEqual(columns);
  expect(moveColumn(columns,"source",1,fixed)).toEqual(columns);
  expect(moveColumn(columns,"nofollow",1,fixed)).toEqual(columns);
  expect(moveColumn(columns,"missing",1,fixed)).toEqual(columns);
});

it("does not save a temporary duplicate-comparison column when another field is reordered",()=>{
  const saved=["address","title","word_count"];
  const required=["address","meta_description"];
  const scoped=visibleColumns(schema,defaults,saved,required);
  expect(scoped).toEqual(["address","meta_description","title","word_count"]);
  const updated=saveColumnOrder(moveColumn(scoped,"word_count",-1,required),saved,required);
  expect(updated).toEqual(["address","word_count","title"]);
  expect(visibleColumns(schema,defaults,updated,["address"])).toEqual(updated);
});

it("retains comparison fields that the user explicitly selected",()=>{
  const base=["address","meta_description","title"];
  const required=["address","meta_description"];
  expect(saveColumnOrder(base,base,required)).toEqual(base);
});
