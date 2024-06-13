'use client'

import {invoke} from "@tauri-apps/api/tauri";
import React, {useEffect, useState} from "react";

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
    };
    sideboard: Card[];
}


interface Mulligan {
    hand: string[];
    opponent_identity: string;
    game_number: number;
    number_to_keep: number;
    play_draw: string;
    decision: string;
}

interface MatchDetails {
    id: number;
    did_controller_win: boolean;
    controller_player_name: string;
    opponent_player_name: string;
    primary_decklist: PrimaryDecklist | null;
    decklists: DeckList[];
    mulligans: Mulligan[];
}

interface Card {
    name: string;
    quantity: number;
    mana_value: number;
}

interface SubTypeListProps {
    header: string;
    cards: Card[];
    includeManaValue: boolean;
}

const SubTypeList: React.FC<SubTypeListProps> = ({ header, cards, includeManaValue }) => {
    return (
        <div>
            <h5>{header}</h5>
            {cards.map((card, index) => (
                <p key={index}>
                    {card.quantity} {card.name}
                    {includeManaValue && ` - ${card.mana_value}`}
                </p>
            ))}
        </div>
    );
};


export default function MatchDetails() {
    const [match, setMatch] = useState<MatchDetails | null>(null);
    useEffect(() => {
        let params = new URLSearchParams(document.location.search);
        let id = params.get("id");
        if (id !== null) {
            invoke<MatchDetails>('get_match_details', {matchId: id})
                .then(result => {
                    console.log(result)
                    setMatch(result);
                })
                .catch(console.error)
        } else {
            setMatch({
                id: 0,
                did_controller_win: false,
                controller_player_name: "",
                opponent_player_name: "",
                primary_decklist: null,
                decklists: [],
                mulligans: []
            });
        }
    }, [])

    // Ensure that all card types are defined because I'm tired of life and javascript deserves to suffer
    if (match && match.primary_decklist) {
        match.primary_decklist.main_deck.Artifact = match.primary_decklist.main_deck.Artifact || [];
        match.primary_decklist.main_deck.Creature = match.primary_decklist.main_deck.Creature || [];
        match.primary_decklist.main_deck.Enchantment = match.primary_decklist.main_deck.Enchantment || [];
        match.primary_decklist.main_deck.Instant = match.primary_decklist.main_deck.Instant || [];
        match.primary_decklist.main_deck.Land = match.primary_decklist.main_deck.Land || [];
        match.primary_decklist.main_deck.Planeswalker = match.primary_decklist.main_deck.Planeswalker || [];
        match.primary_decklist.main_deck.Sorcery = match.primary_decklist.main_deck.Sorcery || [];
        match.primary_decklist.sideboard = match.primary_decklist.sideboard || [];
    }

    return (
        <div>
            <h1>Match Details</h1>
            {match && (
                <div>
                    <div>
                        <p>Match ID: {match.id}</p>
                        <p>Controller: {match.controller_player_name}</p>
                        <p>Opponent: {match.opponent_player_name}</p>
                        <p>Winner: {match.did_controller_win ? match.controller_player_name : match.opponent_player_name}</p>
                    </div>
                    <div>
                        <h2>Decklists</h2>
                        {match.primary_decklist && (
                            <div>
                                <h3>Primary Decklist</h3>
                                <p>Archetype: {match.primary_decklist.archetype}</p>
                                <h4>Main Deck</h4>
                                {match.primary_decklist.main_deck.Creature.length > 0 && (
                                    <SubTypeList header="Creatures" cards={match.primary_decklist.main_deck.Creature} includeManaValue={true} />
                                )}
                                {match.primary_decklist.main_deck.Instant.length > 0 && (
                                    <SubTypeList header="Instants" cards={match.primary_decklist.main_deck.Instant} includeManaValue={true} />
                                )}
                                {match.primary_decklist.main_deck.Sorcery.length > 0 && (
                                    <SubTypeList header="Sorceries" cards={match.primary_decklist.main_deck.Sorcery} includeManaValue={true} />
                                )}
                                {match.primary_decklist.main_deck.Enchantment.length > 0 && (
                                    <SubTypeList header="Enchantments" cards={match.primary_decklist.main_deck.Enchantment} includeManaValue={true} />
                                )}
                                {match.primary_decklist.main_deck.Artifact.length > 0 && (
                                    <SubTypeList header="Artifacts" cards={match.primary_decklist.main_deck.Artifact} includeManaValue={true} />
                                )}
                                {match.primary_decklist.main_deck.Planeswalker.length > 0 && (
                                    <SubTypeList header="Planeswalkers" cards={match.primary_decklist.main_deck.Planeswalker} includeManaValue={true} />
                                )}
                                {match.primary_decklist.main_deck.Land.length > 0 && (
                                    <SubTypeList header="Lands" cards={match.primary_decklist.main_deck.Land} includeManaValue={false} />
                                )}
                                {match.primary_decklist.sideboard.length > 0 && (
                                    <SubTypeList header="Sideboard" cards={match.primary_decklist.sideboard} includeManaValue={true} />
                                )}
                            </div>
                        )}

                        {match.decklists.map((decklist, index) => (
                            <div key={index}>
                                <h3>Game {decklist.game_number}</h3>
                                <p>Deck: {decklist.deck.join(", ")}</p>
                                <p>Sideboard: {decklist.sideboard.join(", ")}</p>
                            </div>
                        ))}
                    </div>
                    <div>
                        <h2>Mulligans</h2>
                        {match.mulligans.map((mulligan, index) => (
                            <div key={index}>
                                <h3>Game {mulligan.game_number}</h3>
                                <p>Hand: {mulligan.hand.join(", ")}</p>
                                <p>Opponent Identity: {mulligan.opponent_identity}</p>
                                <p>Number to Keep: {mulligan.number_to_keep}</p>
                                <p>Play/Draw: {mulligan.play_draw}</p>
                                <p>Decision: {mulligan.decision}</p>
                            </div>
                        ))}
                    </div>
                </div>
            )}
        </div>
    )
}