package com.tomcat.controller;

import cn.hutool.json.JSONUtil;
import com.tomcat.controller.requeset.AuthenticationRequest;
import com.tomcat.domain.User;
import com.tomcat.domain.UserRepository;
import com.tomcat.utils.JwtUtil;
import org.junit.jupiter.api.MethodOrderer;
import org.junit.jupiter.api.Order;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestMethodOrder;
import org.junit.runner.RunWith;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.boot.test.autoconfigure.web.servlet.AutoConfigureMockMvc;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.http.MediaType;
import org.springframework.test.context.junit4.SpringRunner;
import org.springframework.test.web.servlet.MockMvc;
import org.springframework.test.web.servlet.MvcResult;
import org.springframework.test.web.servlet.ResultHandler;
import org.springframework.test.web.servlet.request.MockMvcRequestBuilders;
import org.springframework.test.web.servlet.result.MockMvcResultMatchers;

@SpringBootTest
@RunWith(SpringRunner.class)
@AutoConfigureMockMvc
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
class AuthControllerTest {

    @Autowired
    private MockMvc mockMvc;

    @Autowired
    private JwtUtil jwtUtil;

    @Value("${jwt.tokenHeader}")
    private String tokenHeader;
    @Value("${jwt.tokenPrefix}")
    private String tokenHead;

    @Autowired
    UserRepository userRepository;


    @Test
    @Order(2)
    void findByDeviceIdSuccess() throws Exception{
        User user = userRepository.findByUsername("汤姆猫").orElseThrow(() -> new RuntimeException("用户不存在"));
        AuthenticationRequest request = new AuthenticationRequest(user.getUsername(), "", user.getEmail(), user.getPhoneNum(), user.getDeviceId());
        String token = jwtUtil.generateToken(user.getId(), request.username);
        String json = JSONUtil.toJsonStr(request);
        mockMvc.perform(MockMvcRequestBuilders
                        .get("/auth/findByDeviceId?deviceId="+request.deviceId)
                        .content(json.getBytes())
                        .contentType(MediaType.APPLICATION_JSON_VALUE)
                        .header(tokenHeader,tokenHead+token)
                ).andExpect(MockMvcResultMatchers.status().isOk())
                .andDo(new ResultHandler() {
                    @Override
                    public void handle(MvcResult result) throws Exception {
                        System.out.println("Response body: " + result.getResponse().getContentAsString());
                    }
                });
    }


    @Test
    @Order(3)
    void findByDeviceIdFail() throws Exception{
        // 模拟没有token的请求
        AuthenticationRequest request = new AuthenticationRequest("tom", "1234", "", "13823232232", "8888888");
        String json = JSONUtil.toJsonStr(request);
        mockMvc.perform(MockMvcRequestBuilders
                .post("/auth/findByDeviceId")
                .content(json.getBytes())
                .accept(MediaType.APPLICATION_JSON)
                .contentType(MediaType.APPLICATION_JSON_VALUE)
        ).andExpect(MockMvcResultMatchers.status().is(403))
                .andDo(new ResultHandler() {
            @Override
            public void handle(MvcResult result) throws Exception {
                System.out.println("Response body: " + result.getResponse().getContentAsString());
            }
        });
    }


    @Test
    @Order(1)
    void registerOrLogin() throws Exception{
//        AuthenticationRequest request = new AuthenticationRequest("tom1", "1234", "431@qq.com", "13823232231", "8888887");
        AuthenticationRequest request = new AuthenticationRequest("汤姆猫", "1234", "431@qq.com", "13823232231", "8888887");
        String json = JSONUtil.toJsonStr(request);
        mockMvc.perform(MockMvcRequestBuilders
                .post("/auth/registerOrLogin")
                .content(json.getBytes())
                .accept(MediaType.APPLICATION_JSON)
                .contentType(MediaType.APPLICATION_JSON_VALUE)
        ).andExpect(MockMvcResultMatchers.status().isOk())
                .andDo(new ResultHandler() {
                    @Override
                    public void handle(MvcResult result) throws Exception {
                        System.out.println("Response body: " + result.getResponse().getContentAsString());
                    }
                });
    }
}