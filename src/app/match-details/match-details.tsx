"use client";

import { invoke } from "@tauri-apps/api/core";
import React, { useEffect, useState } from "react";

interface DeckList {
  game_number: number;
  deck: number[];
  sideboard: number[];
}

interface Card {
  name: string;
  quantity: number;
  mana_value: number;
}

interface PrimaryDecklist {
  archetype: string;
  main_deck: {
    Creature: Card[];
    Instant: Card[];
    Sorcery: Card[];
    Enchantment: Card[];
    Artifact: Card[];
    Planeswalker: Card[];
    Land: Card[];
    Unknown: Card[];
  };
  sideboard: Card[];
}

interface Mulligan {
  hand: Card[];
  opponent_identity: string;
  game_number: number;
  number_to_keep: number;
  play_draw: string;
  decision: string;
}

interface DeckDifference {
  added: Card[];
  removed: Card[];
}

interface GameResult {
  game_number: number;
  winning_player: string;
}

interface MatchDetails {
  id: number;
  did_controller_win: boolean;
  controller_player_name: string;
  opponent_player_name: string;
  created_at: string;
  primary_decklist: PrimaryDecklist | null;
  game_results: GameResult[];
  differences: DeckDifference[] | null;
  decklists: DeckList[];
  mulligans: Mulligan[];
}

interface Card {
  name: string;
  quantity: number;
  mana_value: number;
  image_uri: string;
}

interface CardEntryProps {
  identifier: string;
  key: number;
  card: Card;
  includeManaValue: boolean;
}

interface SubTypeListProps {
  header: string;
  cards: Card[];
  includeManaValue: boolean;
}

const SubTypeList: React.FC<SubTypeListProps> = ({
  header,
  cards,
  includeManaValue,
}) => {
  return (
    <div className={"flex-wrap p-2 grid"}>
      <h5 className="text-lg font-semibold">{header}</h5>
      {cards.map((card, index) => (
        <CardEntry identifier={`${header}-${index}`} key={index} card={card} includeManaValue={includeManaValue} />
      ))}
    </div>
  );
};

const CardEntry: React.FC<CardEntryProps> = ({ identifier, key, card, includeManaValue }) => {
  return (
    <div id={identifier} key={key} className="flex flex-row">
      <p className="hover:cursor-pointer relative group"
           onMouseEnter={() => {
             const img = document.createElement('img');
             const containerDiv = document.getElementById(identifier);
             if (containerDiv === null) return;
             const child = containerDiv.children[0];
             const rect = child.getBoundingClientRect()
             const x = (rect ? rect.right: 0) + 10;
             const y = (rect ? rect.top: 0) + 10;
             img.style.zIndex = '1000';
             img.style.position = 'fixed';
             img.src = card.image_uri;
             img.className = `absolute h-auto w-auto0`;
             img.style.top = `${y}px`;
             img.style.left = `${x}px`;
             img.style.maxWidth = '200px';
             img.style.maxHeight = '300px';
             img.alt = card.name;
             img.id = `hover-image-${card.name}`;
             containerDiv.appendChild(img);
           }}
           onMouseLeave={() => {
             const img = document.getElementById(`hover-image-${card.name}`);
             if (img) img.remove();
           }}>
        {card.quantity} {card.name}
          {includeManaValue && ` - ${card.mana_value}`}
      </p>
    </div>
  );
};


export default function MatchDetails() {
  const [match, setMatch] = useState<MatchDetails | null>(null);
  useEffect(() => {
    let params = new URLSearchParams(document.location.search);
    let id = params.get("id");
    if (id !== null) {
      invoke<MatchDetails>("command_match_details", { matchId: id })
        .then((result) => {
          console.log(result);
          setMatch(result);
        })
        .catch(console.error);
    } else {
      setMatch({
        id: 0,
        did_controller_win: false,
        controller_player_name: "",
        opponent_player_name: "",
        created_at: "",
        primary_decklist: null,
        game_results: [],
        differences: null,
        decklists: [],
        mulligans: [],
      });
    }
  }, []);

  // Ensure that all card types are defined because I'm tired of life and javascript deserves to suffer
  if (match && match.primary_decklist) {
    match.primary_decklist.main_deck.Artifact =
      match.primary_decklist.main_deck.Artifact || [];
    match.primary_decklist.main_deck.Creature =
      match.primary_decklist.main_deck.Creature || [];
    match.primary_decklist.main_deck.Enchantment =
      match.primary_decklist.main_deck.Enchantment || [];
    match.primary_decklist.main_deck.Instant =
      match.primary_decklist.main_deck.Instant || [];
    match.primary_decklist.main_deck.Land =
      match.primary_decklist.main_deck.Land || [];
    match.primary_decklist.main_deck.Planeswalker =
      match.primary_decklist.main_deck.Planeswalker || [];
    match.primary_decklist.main_deck.Sorcery =
      match.primary_decklist.main_deck.Sorcery || [];
    match.primary_decklist.main_deck.Unknown =
      match.primary_decklist.main_deck.Unknown || [];
    match.primary_decklist.sideboard = match.primary_decklist.sideboard || [];
  }

  var mulligans: Mulligan[][] = [[], [], []];
  if (match && match.mulligans) {
    for (var i = 0; i < match.mulligans.length; i++) {
      while (match.mulligans[i].game_number >= mulligans.length + 1) {
        mulligans.push([]);
      }
      mulligans[match.mulligans[i].game_number - 1].push(match.mulligans[i]);
    }
  }

  return (
    <div className="container mx-auto px-4">
      {match && (
        <div>
          <div>
            <h1 className="text-2xl mx-auto px-4">
              VS. {match.opponent_player_name}
            </h1>
            <p className="float-right">{match.id}</p>
            <p>{match.created_at}</p>
            <p>Controller: {match.controller_player_name}</p>
            <p>Opponent: {match.opponent_player_name}</p>
            <p>
              Winner:{" "}
              {match.did_controller_win
                ? match.controller_player_name
                : match.opponent_player_name}
            </p>
            <div>
              {match.game_results.map((game_result, index) => (
                <p key={index}>
                  Game {game_result.game_number}: {game_result.winning_player}
                </p>
              ))}
            </div>
          </div>
          <h3 className="text-lg font-bold mb-1">Primary Decklist</h3>
          <div className="grid grid-cols-3 gap-4">
            {match.primary_decklist && (
              <div>
                <p>Archetype: {match.primary_decklist.archetype}</p>
                <h4>Main Deck</h4>
                <div>
                  {match.primary_decklist.main_deck.Creature.length > 0 && (
                    <SubTypeList
                      header="Creatures"
                      cards={match.primary_decklist.main_deck.Creature}
                      includeManaValue={true}
                    />
                  )}
                  {match.primary_decklist.main_deck.Instant.length > 0 && (
                    <SubTypeList
                      header="Instants"
                      cards={match.primary_decklist.main_deck.Instant}
                      includeManaValue={true}
                    />
                  )}
                  {match.primary_decklist.main_deck.Sorcery.length > 0 && (
                    <SubTypeList
                      header="Sorceries"
                      cards={match.primary_decklist.main_deck.Sorcery}
                      includeManaValue={true}
                    />
                  )}
                  {match.primary_decklist.main_deck.Enchantment.length > 0 && (
                    <SubTypeList
                      header="Enchantments"
                      cards={match.primary_decklist.main_deck.Enchantment}
                      includeManaValue={true}
                    />
                  )}
                  {match.primary_decklist.main_deck.Artifact.length > 0 && (
                    <SubTypeList
                      header="Artifacts"
                      cards={match.primary_decklist.main_deck.Artifact}
                      includeManaValue={true}
                    />
                  )}
                </div>
              </div>
            )}
            {match.primary_decklist && (
              <div>
                {match.primary_decklist.main_deck.Planeswalker.length > 0 && (
                  <SubTypeList
                    header="Planeswalkers"
                    cards={match.primary_decklist.main_deck.Planeswalker}
                    includeManaValue={true}
                  />
                )}
                {match.primary_decklist.main_deck.Land.length > 0 && (
                  <SubTypeList
                    header="Lands"
                    cards={match.primary_decklist.main_deck.Land}
                    includeManaValue={false}
                  />
                )}
                {match.primary_decklist.main_deck.Unknown.length > 0 && (
                  <SubTypeList
                    header="Unknown"
                    cards={match.primary_decklist.main_deck.Unknown}
                    includeManaValue={true}
                  />
                )}
                {match.primary_decklist.sideboard.length > 0 && (
                  <SubTypeList
                    header="Sideboard"
                    cards={match.primary_decklist.sideboard}
                    includeManaValue={true}
                  />
                )}
              </div>
            )}

            {match.differences && (
              <div>
                <h3>Sideboard Decisions</h3>
                <div className="grid grid-cols-1 gap-2">
                  {match.differences.map((difference, game_idx) => (
                    <div key={game_idx}>
                      <h4>Game {game_idx + 2}</h4>
                      <h5>Added</h5>
                      {difference.added.map((card, index) => (
                        <CardEntry identifier={`Sideboard-added-${game_idx}-${index}`} key={index} card={card} includeManaValue={false} />
                      ))}
                      <h5>Removed</h5>
                      {difference.removed.map((card, index) => (
                        <CardEntry identifier={`Sideboard-removed-${game_idx}-${index}`} key={index} card={card} includeManaValue={false} />
                      ))}
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
          <div>
            <h2 className="text-xl font-bold mb-2">Mulligans</h2>
            <div className="grid grid-cols-3 gap-4">
              {mulligans.map((mulligan_list, game_idx) => (
                <div key={game_idx}>
                  {mulligan_list.map((mulligan, mulligan_idx) => (
                    <div key={mulligan_idx}>
                      <h3>Game {mulligan.game_number}</h3>
                      <p>Hand</p>
                      {mulligan.hand.map((card, index) => (
                        <CardEntry identifier={`mulligan-${game_idx}-${mulligan_idx}-${index}`} key={index} card={card} includeManaValue={false} />
                      ))}
                      <p>Opponent Identity: {mulligan.opponent_identity}</p>
                      <p>Number to Keep: {mulligan.number_to_keep}</p>
                      <p>Play/Draw: {mulligan.play_draw}</p>
                      <p>Decision: {mulligan.decision}</p>
                    </div>
                  ))}
                </div>
              ))}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
