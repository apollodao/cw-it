FROM osmolabs/osmosis:10.1.1 as build

# reverting nonroot since testcontainers need root
FROM debian

COPY --from=build /bin/osmosisd /bin/osmosisd
COPY ./ci/configs/osmosis/config /osmosis/.osmosisd/config
COPY ./ci/configs/osmosis/data/priv_validator_state.json /osmosis/.osmosisd/data/priv_validator_state.json

ENV HOME /osmosis
WORKDIR $HOME

EXPOSE 26650
EXPOSE 26656
EXPOSE 26657
EXPOSE 26658
EXPOSE 1317
EXPOSE 9090
EXPOSE 9091
EXPOSE 8080
EXPOSE 6060

ENTRYPOINT ["osmosisd"]
CMD [ "start" ]
