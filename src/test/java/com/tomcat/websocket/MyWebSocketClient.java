package com.tomcat.websocket;

import cn.hutool.core.lang.TypeReference;
import cn.hutool.core.util.StrUtil;
import cn.hutool.json.JSONUtil;
import com.tomcat.controller.response.ChatAssistantData;
import com.tomcat.controller.response.ChatResp;
import com.tomcat.controller.response.Topics;
import lombok.extern.slf4j.Slf4j;
import org.java_websocket.client.WebSocketClient;
import org.java_websocket.handshake.ServerHandshake;

import java.net.URI;

@Slf4j
public class MyWebSocketClient extends WebSocketClient {

    public ChatResp<String> chatInitResp;
    public ChatResp<Topics> planResp;

    public ChatResp<ChatAssistantData> startTopicResp;

    public ChatResp<ChatAssistantData> chatTopicResp;

    public MyWebSocketClient(URI uri) {
        super(uri);
    }

    @Override
    public void onOpen(ServerHandshake serverHandshake) {
        log.info("客户端连接成功");
    }

    @Override
    public void onMessage(String msg) {
        log.info("客户端接收到消息：" + msg);
        ChatResp req = JSONUtil.toBean(msg, ChatResp.class);
        switch (req.getCommand()){
            case Command.CHAT_INIT:
                chatInitResp = JSONUtil.toBean(msg, new TypeReference<ChatResp<String>>(){}, false);
                break;
            case Command.CURRICULUM_PLAN:
                planResp = JSONUtil.toBean(msg, new TypeReference<ChatResp<Topics>>(){}, false);
                break;
            case Command.START_TOPIC:
                startTopicResp = JSONUtil.toBean(msg, new TypeReference<ChatResp<ChatAssistantData> >(){}, false);
                break;
            case Command.CHAT:
                chatTopicResp = JSONUtil.toBean(msg, new TypeReference<ChatResp<ChatAssistantData> >(){}, false);
                break;
        }


    }

    @Override
    public void onClose(int i, String s, boolean b) {
        log.info("客户端关闭成功");
    }

    @Override
    public void onError(Exception e) {
        log.error("客户端出错");
    }


}
