package com.tomcat.websocket;

import cn.hutool.core.util.StrUtil;
import cn.hutool.json.JSONUtil;
import com.tomcat.config.LocalCache;
import com.tomcat.nettyws.pojo.Session;
import com.tomcat.utils.JwtUtil;
import com.unfbx.chatgpt.OpenAiClient;
import com.unfbx.chatgpt.entity.chat.ChatCompletionResponse;
import com.unfbx.chatgpt.entity.chat.Message;
import io.netty.handler.codec.http.websocketx.TextWebSocketFrame;
import lombok.extern.slf4j.Slf4j;

import java.util.ArrayList;
import java.util.List;

@Slf4j
public class MessageProcessor {

    OpenAiClient openAiClient;
    Session session;


    public MessageProcessor(OpenAiClient openAiClient, Session session){
        this.openAiClient = openAiClient;
        this.session = session;
    }


    public void processor(String msg) {

        //TODO
        //        检查缓存中是否存在上下文
        //        请求openai API 获得返回数据
        //        推送消息到订阅用户的设备上
        //        异步生成消息摘要用于搜索
        String uid = session.getAttribute(JwtUtil.KEY_USER_ID);
        log.info("processor userId:" + uid);
        String messageContext = (String) LocalCache.CACHE.get(uid);
        List<Message> messages = new ArrayList<>();
        if (StrUtil.isNotBlank(messageContext)) {
            messages = JSONUtil.toList(messageContext, Message.class);
            if (messages.size() >= 10) {
                messages = messages.subList(1, 10);
            }
            Message currentMessage = Message.builder().content(msg).role(Message.Role.USER).build();
            messages.add(currentMessage);
        } else {
            Message currentMessage = Message.builder().content(msg).role(Message.Role.USER).build();
            messages.add(currentMessage);
        }
        ChatCompletionResponse response = openAiClient.chatCompletion(messages);
        Message responseMag = response.getChoices().get(0).getMessage();
        session.sendText(new TextWebSocketFrame("服务器消息 " + uid + "：" + JSONUtil.toJsonStr(responseMag)));
        LocalCache.CACHE.put(uid, JSONUtil.toJsonStr(messages), LocalCache.TIMEOUT);
    }


}
